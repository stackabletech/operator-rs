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
}

pub trait Configuration {
    type Configurable;

    // TODO: Not sure if I need the role_name here
    // TODO: We need to pass in the existing config from parents to run validation checks and we should probably also pass in a "final" parameter or have another "finalize" method callback
    //  one for each role group, one for each role and one for all of it...
    fn compute_env(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, String>, ConfigError>;

    fn compute_cli(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, String>, ConfigError>;

    fn compute_properties(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
        file: &str,
    ) -> Result<BTreeMap<String, String>, ConfigError>;
}

// This deep map causes problems with clippy and rustfmt.
pub type RoleConfigByPropertyKind =
    HashMap<String, HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, String>>>>;
///
/// Given the configuration parameters of all `roles` partition them by `PropertyNameKind` and
/// merge them with the role groups configuration parameters.
///
/// The output is a map keyed by the role names. The value is also a map keyed by role group names and
/// the values are the merged configuration properties "bucketed" by `PropertyNameKind`.
/// # Arguments
/// - `resource`         - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `role_information` - A map keyed by role names. The value is a vector of `PropertyNameKind`
/// - `roles`            - A map keyed by role names.
///
pub fn transform_all_roles_to_config<T>(
    resource: &T::Configurable,
    role_information: HashMap<String, Vec<PropertyNameKind>>,
    roles: HashMap<String, Role<T>>,
) -> RoleConfigByPropertyKind
where
    T: Configuration,
{
    let mut result = HashMap::new();

    for (role_name, role) in roles {
        let role_properties = transform_role_to_config(
            resource,
            &role_name,
            &role,
            // TODO: What to do when role_name not in role_information
            role_information.get(&role_name).unwrap(),
        );
        result.insert(role_name, role_properties);
    }

    result
}

// TODO: boolean flags suck, move ignore_warn to be a flag
pub fn process_validation_result(
    validation_result: &HashMap<String, PropertyValidationResult>,
    ignore_warn: bool,
    ignore_err: bool,
) -> HashMap<String, String> {
    let mut properties = HashMap::new();
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
            PropertyValidationResult::Warn(value, err) => {
                warn!("Property [{}] is set to value [{}] which causes a warning, `ignore_warn` is {}: {:?}", key, value, ignore_warn, err);
                if ignore_warn {
                    properties.insert(key.clone(), value.clone());
                }
            }
            PropertyValidationResult::Error(value, err) => {
                error!(
                    "Property [{}] causes a validation error, will not set: {:?}",
                    key, err
                );
                if ignore_err {
                    properties.insert(key.clone(), value.clone());
                }
                //TODO: Return error
            }
        }
    }
    properties
}

/// Given a single `role`, it generates a data structure suitable for applying a
/// product configuration.
/// The configuration objects of the role groups contained in the given `role` are
/// merged with that of the `role` it's self.
/// In addition, the `*_overrides` settings are also merged in the resulting configuration
/// with the highest priority.
/// The merge priority chain looks like this:
///
/// group overrides -> group config -> role overrides -> role config
///
/// where '->' means "overwrites if existing or adds".
///
/// The output is a map with one entry, keyed by `role_name` and the value is a map where all
/// configuration properties defined in the `role` are partitioned by `PropertyNameKind`.
/// # Arguments
/// - `resource`       - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `role_name`      - Used as key in the output and to partition the configuration properties.
/// - `role`           - The role for which to transform the configuration parameters.
/// - `property_kinds` - Used as "buckets" to partition the configuration properties by.
///
fn transform_role_to_config<T>(
    resource: &T::Configurable,
    role_name: &str,
    role: &Role<T>,
    property_kinds: &[PropertyNameKind],
) -> HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, String>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();

    let role_properties =
        partition_properties_by_kind(resource, role_name, &role.config, property_kinds);

    // for each role group ...
    for (role_group_name, role_group) in &role.role_groups {
        // ... compute the group properties ...
        let role_group_properties = partition_properties_by_kind(
            resource,
            role_group_name,
            &role_group.config,
            property_kinds,
        );

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

/// Given a `config` object and the `property_kind` vector, it uses the `Configuration::compute_*` methods
/// to partition the configuration properties by `PropertyNameKind`.
///
/// The output is map where the configuration properties are keyed by `PropertyNameKind`.
///
/// # Arguments
/// - `resource`       - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `name`           - Not used directly but passed on to the `Configuration::compute_*` calls. It usually
///                      contains a role name or a role group name.
/// - `config`         - The configuration properties to partition.
/// - `property_kinds` - The "buckets" used to partition the configuration properties.
///
fn partition_properties_by_kind<T>(
    resource: &<T as Configuration>::Configurable,
    name: &str,
    config: &Option<CommonConfiguration<T>>,
    property_kinds: &[PropertyNameKind],
) -> HashMap<PropertyNameKind, BTreeMap<String, String>>
where
    T: Configuration,
{
    let mut result = HashMap::new();

    for property_kind in property_kinds {
        match property_kind {
            PropertyNameKind::File(file) => result.insert(
                property_kind.clone(),
                parse_conf_properties(resource, name, config, file),
            ),
            PropertyNameKind::Env => result.insert(
                property_kind.clone(),
                parse_env_properties(resource, name, config),
            ),
            PropertyNameKind::Cli => result.insert(
                property_kind.clone(),
                parse_cli_properties(resource, name, config),
            ),
        };
    }
    result
}

fn parse_cli_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> BTreeMap<String, String>
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
            final_properties.insert(key.clone(), value.clone());
        }
    }

    final_properties
}

fn parse_env_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> BTreeMap<String, String>
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
            final_properties.insert(key.clone(), value.clone());
        }
    }

    final_properties
}

// TODO: Can we pass a callback instead of "file" so we can merge all parse_* methods?
fn parse_conf_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
    file: &str,
) -> BTreeMap<String, String>
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
            .compute_properties(resource, role_name, file)
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
                final_properties.insert(key.clone(), value.clone());
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
        ) -> Result<BTreeMap<String, String>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(env) = &self.env {
                result.insert("env".to_string(), env.to_string());
            }
            Ok(result)
        }

        fn compute_cli(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
        ) -> Result<BTreeMap<String, String>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(cli) = &self.cli {
                result.insert("cli".to_string(), cli.to_string());
            }
            Ok(result)
        }

        fn compute_properties(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
            _file: &str,
        ) -> Result<BTreeMap<String, String>, ConfigError> {
            let mut result = BTreeMap::new();
            if let Some(conf) = &self.conf {
                result.insert("conf".to_string(), conf.to_string());
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
                    "env".to_string() => GROUP_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, true, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, true, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, true, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                }
            }
        }
    )]
    #[case(true, false, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => ROLE_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, false, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => ROLE_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, false, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => ROLE_ENV.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(true, false, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => ROLE_ENV.to_string(),
                }
            }
        }
    )]
    #[case(false, true, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(false, true, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(false, true, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(false, true, false, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => GROUP_ENV.to_string(),
                }
            }
        }
    )]
    #[case(false, false, true, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(false, false, true, false,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    ROLE_ENV_OVERRIDE.to_string() => ROLE_ENV_OVERRIDE.to_string(),
                }
            }
        }
    )]
    #[case(false, false, false, true,
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string(),
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
        #[case] expected: HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, String>>>,
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
                        "conf".to_string() => "conf".to_string()
                    ),
                PropertyNameKind::Env =>
                    collection!(
                        "env".to_string() => GROUP_ENV.to_string(),
                        GROUP_ENV_OVERRIDE.to_string() => GROUP_ENV_OVERRIDE.to_string()
                    ),
                PropertyNameKind::Cli =>
                    collection!(
                        "cli".to_string() => GROUP_CLI.to_string(),
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

        let role_information: HashMap<String, Vec<PropertyNameKind>> = collection! {
            role_1.to_string() => vec![PropertyNameKind::File(file_name.to_string()), PropertyNameKind::Env],
            role_2.to_string() => vec![PropertyNameKind::Cli]
        };

        let roles: HashMap<String, Role<TestConfig>> = collection! {
            role_1.to_string() => Role {
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

        },
        role_2.to_string() => Role {
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
        }};

        let expected: RoleConfigByPropertyKind = collection! {
        role_1.to_string() => collection!{
            role_group_1.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => GROUP_ENV.to_string()
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "conf".to_string() => GROUP_CONFIG.to_string()
                }
            },
            role_group_2.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => GROUP_ENV.to_string()
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "conf".to_string() => GROUP_CONFIG.to_string()
                }
            }
        },
        role_2.to_string() => collection! {
            role_group_1.to_string() => collection! {
                PropertyNameKind::Cli => collection! {
                    "cli".to_string() => GROUP_CLI.to_string()
                }
            }
        }};

        let all_config = transform_all_roles_to_config(&String::new(), role_information, roles);

        assert_eq!(all_config, expected);
    }
}
