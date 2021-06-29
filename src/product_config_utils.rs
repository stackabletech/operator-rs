use crate::role_utils::{CommonConfiguration, Role};
use product_config::types::PropertyNameKind;
use product_config::PropertyValidationResult;
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration found")]
    InvalidConfiguration,

    #[error("Collected product config validation errors: {collected_errors:?}")]
    ProductConfigErrors {
        collected_errors: Vec<product_config::error::Error>,
    },
}

/// This trait is used to compute configuration properties for products.
///
/// This needs to be implemented for every T in the [`crate::role_utils::CommonConfig`] struct
/// that is used in [`crate::role_utils::Role`] or the top level (cluster wide) configuration.
///
/// Each `compute_*` method is then called to determine where and how (see options below)
/// config properties are configured within the product.
///
/// The options are:
/// - Environmental variables (env)
/// - Command line arguments (cli)
/// - Configuration files (files)
///
/// Returned empty Maps will be ignored.
///
/// Check out [`crate::ser::to_hash_map`] if you do need to convert a struct to a HashMap
/// in an easy way.
pub trait Configuration {
    type Configurable;

    // TODO: We need to pass in the existing config from parents to run validation checks and we should probably also pass in a "final" parameter or have another "finalize" method callback
    //  one for each role group, one for each role and one for all of it...
    fn compute_env(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>, ConfigError>;

    fn compute_cli(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>, ConfigError>;

    fn compute_files(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
        file: &str,
    ) -> Result<BTreeMap<String, Option<String>>, ConfigError>;
}

// This deep map causes problems with clippy and fmt.
pub type RoleConfigByPropertyKind =
    HashMap<String, HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>>;

/// Given the configuration parameters of all `roles` partition them by `PropertyNameKind` and
/// merge them with the role groups configuration parameters.
///
/// The output is a map keyed by the role names. The value is also a map keyed by role group names and
/// the values are the merged configuration properties "bucketed" by `PropertyNameKind`.
///
/// # Arguments
/// - `resource`         - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `roles`            - A map keyed by role names. The value is a tuple of the [`crate::role_utils::Role`] and
///                        required PropertyNameKind like (Cli, Env or Files).
pub fn transform_all_roles_to_config<T>(
    resource: &T::Configurable,
    roles: HashMap<String, (Role<T>, Vec<PropertyNameKind>)>,
) -> RoleConfigByPropertyKind
where
    T: Configuration,
{
    let mut result = HashMap::new();

    for (role_name, (role, property_name_kinds)) in &roles {
        let role_properties =
            transform_role_to_config(resource, role_name, role, property_name_kinds);
        result.insert(role_name.to_string(), role_properties);
    }

    result
}

/// This transforms the [`product_config::types::PropertyValidationResult`] back into a pure BTreeMap which can be used
/// to set properties for config files, cli or environmental variables.
/// Default values are ignored, Recommended and Valid values are used as is. For Warning and
/// Error we recommend to not use the values unless you really know what you are doing.
/// If you want to use the values anyways please check the "ignore_warn" and "ignore_err" switches.
///
/// # Arguments
/// - `validation_result`   - The product config validation result for each property name.
/// - `ignore_warn`         - A switch to ignore product config warnings and continue with
///                           the value anyways. Not recommended!
/// - `ignore_err`          - A switch to ignore product config errors and continue with
///                           the value anyways. Not recommended!
// TODO: boolean flags suck, move ignore_warn to be a flag
pub fn process_validation_result(
    validation_result: &HashMap<String, PropertyValidationResult>,
    ignore_warn: bool,
    ignore_err: bool,
) -> Result<BTreeMap<String, String>, ConfigError> {
    let mut properties = BTreeMap::new();
    let mut collected_errors = Vec::new();

    for (key, result) in validation_result.iter() {
        match result {
            PropertyValidationResult::Default(value) => {
                debug!("Property [{}] is not explicitly set, will not set and rely on the default instead ([{}])", key, value);
            }
            PropertyValidationResult::RecommendedDefault(value) => {
                debug!(
                    "Property [{}] is not set, will use recommended default [{}] instead",
                    key, value
                );
                properties.insert(key.clone(), value.clone());
            }
            PropertyValidationResult::Valid(value) => {
                debug!("Property [{}] is set to valid value [{}]", key, value);
                properties.insert(key.clone(), value.clone());
            }
            PropertyValidationResult::Override(value) => {
                debug!("Property [{}] is set to override value [{}]", key, value);
                properties.insert(key.clone(), value.clone());
            }
            PropertyValidationResult::Warn(value, err) => {
                warn!("Property [{}] is set to value [{}] which causes a warning, `ignore_warn` is {}: {:?}", key, value, ignore_warn, err);
                if ignore_warn {
                    properties.insert(key.clone(), value.clone());
                }
            }
            PropertyValidationResult::Error(value, err) => {
                error!("Property [{}] is set to value [{}] which causes an error, `ignore_err` is {}: {:?}", key, value, ignore_err, err);
                if ignore_err {
                    properties.insert(key.clone(), value.clone());
                } else {
                    collected_errors.push(err.clone());
                }
            }
        }
    }

    if !collected_errors.is_empty() {
        return Err(ConfigError::ProductConfigErrors { collected_errors });
    }

    Ok(properties)
}

/// Given a single [`crate::role_utils::Role`], it generates a data structure that can be validated in the
/// product configuration. The configuration objects of the [`crate::role_utils::RoleGroup] contained in the
/// given [`crate::role_utils::Role`] are merged with that of the [`crate::role_utils::Role`] itself.
/// In addition, the `*_overrides` settings are also merged in the resulting configuration
/// with the highest priority. The merge priority chain looks like this where '->' means
/// "overwrites if existing or adds":
///
/// group overrides -> group config -> role overrides -> role config (TODO: -> common_config)
///
/// The output is a map where the [`crate::role_utils::RoleGroup] name points to another map of
/// [`product_config::types::PropertyValidationResult`] that points to the mapped configuration
/// properties in the (possibly overridden) [`crate::role_utils::Role`] and [`crate::role_utils::RoleGroup].
///
/// # Arguments
/// - `resource`       - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `role_name`      - The name of the role.
/// - `role`           - The role for which to transform the configuration parameters.
/// - `property_kinds` - Used as "buckets" to partition the configuration properties by.
fn transform_role_to_config<T>(
    resource: &T::Configurable,
    role_name: &str,
    role: &Role<T>,
    property_kinds: &[PropertyNameKind],
) -> HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();

    let role_properties = parse_role_config(resource, role_name, &role.config, property_kinds);

    // for each role group ...
    for (role_group_name, role_group) in &role.role_groups {
        // ... compute the group properties ...
        let role_group_properties =
            parse_role_config(resource, role_name, &role_group.config, property_kinds);

        // ... and merge them with the role properties.
        let mut role_properties_copy = role_properties.clone();
        for (property_kind, properties) in role_group_properties {
            role_properties_copy
                .entry(property_kind)
                .or_default()
                .extend(properties);
        }

        result.insert(role_group_name.clone(), role_properties_copy);
    }

    result
}

/// Given a `config` object and the `property_kinds` vector, it uses the `Configuration::compute_*` methods
/// to partition the configuration properties by [`product_config::types::PropertyValidationResult`].
///
/// The output is a map where the configuration properties are keyed by [`product_config::types::PropertyValidationResult`].
///
/// # Arguments
/// - `resource`       - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `role_name`      - Not used directly but passed on to the `Configuration::compute_*` calls.
/// - `config`         - The configuration properties to partition.
/// - `property_kinds` - The "buckets" used to partition the configuration properties.
fn parse_role_config<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
    property_kinds: &[PropertyNameKind],
) -> HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();

    for property_kind in property_kinds {
        match property_kind {
            PropertyNameKind::File(file) => result.insert(
                property_kind.clone(),
                parse_file_properties(resource, role_name, config, file),
            ),
            PropertyNameKind::Env => result.insert(
                property_kind.clone(),
                parse_env_properties(resource, role_name, config),
            ),
            PropertyNameKind::Cli => result.insert(
                property_kind.clone(),
                parse_cli_properties(resource, role_name, config),
            ),
        };
    }
    result
}

fn parse_cli_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> BTreeMap<String, Option<String>>
where
    T: Configuration,
{
    let mut final_properties = BTreeMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref config),
        ..
    }) = config
    {
        final_properties = config.compute_cli(resource, role_name).unwrap();
    }

    // ...followed by config_overrides from the role
    if let Some(CommonConfiguration {
        cli_overrides: Some(ref config),
        ..
    }) = config
    {
        for (key, value) in config {
            final_properties.insert(key.clone(), Some(value.clone()));
        }
    }

    final_properties
}

fn parse_env_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> BTreeMap<String, Option<String>>
where
    T: Configuration,
{
    let mut final_properties = BTreeMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref config),
        ..
    }) = config
    {
        final_properties = config.compute_env(resource, role_name).unwrap();
    }

    // ...followed by config_overrides from the role
    if let Some(CommonConfiguration {
        env_overrides: Some(ref config),
        ..
    }) = config
    {
        for (key, value) in config {
            final_properties.insert(key.clone(), Some(value.clone()));
        }
    }

    final_properties
}

fn parse_file_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
    file: &str,
) -> BTreeMap<String, Option<String>>
where
    T: Configuration,
{
    let mut final_properties = BTreeMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref inner_config),
        ..
    }) = config
    {
        final_properties = inner_config
            .compute_files(resource, role_name, file)
            .unwrap();
    }

    // ...followed by config_overrides from the role
    if let Some(CommonConfiguration {
        config_overrides: Some(ref inner_config),
        ..
    }) = config
    {
        // For Conf files only process overrides that match our file name
        if let Some(config) = inner_config.get(file) {
            for (key, value) in config {
                final_properties.insert(key.clone(), Some(value.clone()));
            }
        }
    }

    final_properties
}

#[cfg(test)]
mod tests {
    macro_rules! collection {
        // map-like
        ($($k:expr => $v:expr),* $(,)?) => {
            std::iter::Iterator::collect(std::array::IntoIter::new([$(($k, $v),)*]))
        };
        // set-like
        ($($v:expr),* $(,)?) => {
            std::iter::Iterator::collect(std::array::IntoIter::new([$($v,)*]))
        };
    }

    use super::*;
    use crate::role_utils::{Role, RoleGroup};
    use rstest::*;
    use std::collections::HashMap;
    const ROLE_GROUP: &str = "role_group";

    const ROLE_CONFIG: &str = "role_config";
    const ROLE_ENV: &str = "role_env";
    const ROLE_CLI: &str = "role_cli";

    const GROUP_CONFIG: &str = "group_config";
    const GROUP_ENV: &str = "group_env";
    const GROUP_CLI: &str = "group_cli";

    const ROLE_CONF_OVERRIDE: &str = "role_conf_override";
    const ROLE_ENV_OVERRIDE: &str = "role_env_override";
    const ROLE_CLI_OVERRIDE: &str = "role_cli_override";

    const GROUP_CONF_OVERRIDE: &str = "group_conf_override";
    const GROUP_ENV_OVERRIDE: &str = "group_env_override";
    const GROUP_CLI_OVERRIDE: &str = "group_cli_override";

    #[derive(Clone, Debug, PartialEq)]
    struct TestConfig {
        pub conf: Option<String>,
        pub env: Option<String>,
        pub cli: Option<String>,
    }

    impl Configuration for TestConfig {
        type Configurable = String;

        fn compute_env(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
        ) -> Result<BTreeMap<String, Option<String>>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(env) = &self.env {
                result.insert("env".to_string(), Some(env.to_string()));
            }
            Ok(result)
        }

        fn compute_cli(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
        ) -> Result<BTreeMap<String, Option<String>>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(cli) = &self.cli {
                result.insert("cli".to_string(), Some(cli.to_string()));
            }
            Ok(result)
        }

        fn compute_files(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
            _file: &str,
        ) -> Result<BTreeMap<String, Option<String>>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(conf) = &self.conf {
                result.insert("conf".to_string(), Some(conf.to_string()));
            }
            Ok(result)
        }
    }

    fn build_test_config(conf: &str, env: &str, cli: &str) -> Option<TestConfig> {
        Some(TestConfig {
            conf: Some(conf.to_string()),
            env: Some(env.to_string()),
            cli: Some(cli.to_string()),
        })
    }

    fn build_common_config(
        test_config: Option<TestConfig>,
        config_overrides: Option<HashMap<String, HashMap<String, String>>>,
        env_overrides: Option<HashMap<String, String>>,
        cli_overrides: Option<BTreeMap<String, String>>,
    ) -> Option<CommonConfiguration<TestConfig>> {
        Some(CommonConfiguration {
            config: test_config,
            config_overrides,
            env_overrides,
            cli_overrides,
        })
    }

    fn build_config_override(
        file_name: &str,
        property: &str,
    ) -> Option<HashMap<String, HashMap<String, String>>> {
        Some(
            collection!( file_name.to_string() => collection!( property.to_string() => property.to_string())),
        )
    }

    fn build_env_override(property: &str) -> Option<HashMap<String, String>> {
        Some(collection!( property.to_string() => property.to_string()))
    }

    fn build_cli_override(property: &str) -> Option<BTreeMap<String, String>> {
        Some(collection! {property.to_string() => property.to_string()})
    }

    fn build_role_and_group(
        role_config: bool,
        group_config: bool,
        role_overrides: bool,
        group_overrides: bool,
    ) -> Role<TestConfig> {
        let role_group = ROLE_GROUP.to_string();
        let file_name = "foo.conf";

        match (role_config, group_config, role_overrides, group_overrides) {
            (true, true, true, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (true, true, true, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI), None, None, None),
                    selector: None,
                }},
            },
            (true, true, false, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (true, true, false, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                        selector: None,
                }},
            },
            (true, false, true, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (true, false, true, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: None,
                    selector: None,
                }},
            },
            (true, false, false, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)
                    ),
                    selector: None,
                }},
            },
            (true, false, false, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: None,
                    selector: None,
                }},
            },
            (false, true, true, true) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (false, true, true, false) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                        selector: None,
                }},
            },
            (false, true, false, true) => Role {
                config: None,
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (false, true, false, false) => Role {
                config: None,
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                        selector: None,
                }},
            },
            (false, false, true, true) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (false, false, true, false) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: None,
                    selector: None,
                }},
            },
            (false, false, false, true) => Role {
                config: None,
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                        selector: None,
                }},
            },
            (false, false, false, false) => Role {
                config: None,
                role_groups: collection! {role_group => RoleGroup {
                    replicas: 1,
                    config: None,
                    selector: None,
                }},
            },
        }
    }

    #[rstest]
    #[case(true, true, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, true, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, true, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, true, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                }
            }
        }
    )]
    #[case(true, false, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(ROLE_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, false, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(ROLE_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, false, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(ROLE_ENV.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(true, false, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(ROLE_ENV.to_string()),
                }
            }
        }
    )]
    #[case(false, true, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, true, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, true, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, true, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                }
            }
        }
    )]
    #[case(false, false, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, false, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    ROLE_ENV_OVERRIDE.to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, false, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(false, false, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                }
            }
        }
    )]
    #[trace]
    fn test_transform_role_to_config(
        #[case] role_config: bool,
        #[case] group_config: bool,
        #[case] role_overrides: bool,
        #[case] group_overrides: bool,
        #[case] expected: HashMap<
            String,
            HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>,
        >,
    ) {
        let role = build_role_and_group(role_config, group_config, role_overrides, group_overrides);

        let property_kinds = vec![PropertyNameKind::Env];

        let config = transform_role_to_config(&String::new(), ROLE_GROUP, &role, &property_kinds);

        assert_eq!(config, expected);
    }

    #[test]
    fn test_transform_role_to_config_overrides() {
        let role_group = "role_group";
        let file_name = "foo.bar";
        let role = Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                // should override
                build_config_override(file_name, "conf"),
                None,
                // should override
                build_cli_override("cli"),
            ),
            role_groups: collection! {role_group.to_string() => RoleGroup {
                replicas: 1,
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    // should override
                    build_config_override(file_name, "conf"),
                    build_env_override(GROUP_ENV_OVERRIDE),
                    None),
                    selector: None,
            }},
        };

        let expected = collection! {
        role_group.to_string() =>
            collection!{
                PropertyNameKind::File(file_name.to_string()) =>
                    collection!(
                        "conf".to_string() => Some("conf".to_string())
                    ),
                PropertyNameKind::Env =>
                    collection!(
                        "env".to_string() => Some(GROUP_ENV.to_string()),
                        GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string())
                    ),
                PropertyNameKind::Cli =>
                    collection!(
                        "cli".to_string() => Some(GROUP_CLI.to_string()),
                    ),
            }
        };

        let property_kinds = vec![
            PropertyNameKind::File(file_name.to_string()),
            PropertyNameKind::Env,
            PropertyNameKind::Cli,
        ];

        let config = transform_role_to_config(&String::new(), ROLE_GROUP, &role, &property_kinds);

        assert_eq!(config, expected);
    }

    #[test]
    fn test_transform_all_roles_to_config() {
        let role_1 = "role_1";
        let role_2 = "role_2";
        let role_group_1 = "role_group_1";
        let role_group_2 = "role_group_2";
        let file_name = "foo.bar";

        let roles: HashMap<String, (Role<TestConfig>, Vec<PropertyNameKind>)> = collection! {
            role_1.to_string() => (Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                None,
                None,
                None,
            ),
            role_groups: collection! {role_group_1.to_string() => RoleGroup {
                replicas: 1,
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
                selector: None,
            },
            role_group_2.to_string() => RoleGroup {
                replicas: 1,
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
                selector: None,
            }}

        }, vec![PropertyNameKind::File(file_name.to_string()), PropertyNameKind::Env]),
            role_2.to_string() => (Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                None,
                None,
                None,
            ),
            role_groups: collection! {role_group_1.to_string() => RoleGroup {
                replicas: 1,
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
                selector: None,
            }},
        }, vec![PropertyNameKind::Cli])
        };

        let expected: RoleConfigByPropertyKind = collection! {
        role_1.to_string() => collection!{
            role_group_1.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => Some(GROUP_ENV.to_string())
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "conf".to_string() => Some(GROUP_CONFIG.to_string())
                }
            },
            role_group_2.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => Some(GROUP_ENV.to_string())
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "conf".to_string() => Some(GROUP_CONFIG.to_string())
                }
            }
        },
        role_2.to_string() => collection! {
            role_group_1.to_string() => collection! {
                PropertyNameKind::Cli => collection! {
                    "cli".to_string() => Some(GROUP_CLI.to_string())
                }
            }
        }};

        let all_config = transform_all_roles_to_config(&String::new(), roles);

        assert_eq!(all_config, expected);
    }
}
