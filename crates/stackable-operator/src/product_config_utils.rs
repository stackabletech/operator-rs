use std::collections::{BTreeMap, HashMap};

use k8s_openapi::api::core::v1::EnvVar;
use product_config::{ProductConfigManager, PropertyValidationResult, types::PropertyNameKind};
use schemars::JsonSchema;
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use tracing::{debug, error, warn};

use crate::role_utils::{CommonConfiguration, Role};

pub const CONFIG_OVERRIDE_FILE_HEADER_KEY: &str = "EXPERIMENTAL_FILE_HEADER";
pub const CONFIG_OVERRIDE_FILE_FOOTER_KEY: &str = "EXPERIMENTAL_FILE_FOOTER";

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("invalid configuration found"))]
    InvalidConfiguration {
        source: product_config::error::Error,
    },

    #[snafu(display("collected product config validation errors: {collected_errors:?}"))]
    ProductConfigErrors {
        collected_errors: Vec<product_config::error::Error>,
    },

    #[snafu(display("missing role {role:?}. This should not happen. Will requeue."))]
    MissingRole { role: String },

    #[snafu(display(
        "missing roleGroup {role_group:?} for role {role:?}. This might happen after custom resource changes. Will requeue."
    ))]
    MissingRoleGroup { role: String, role_group: String },

    // We need this for product specific errors that implement the Configuration trait and are not related to the
    // product config. This allows us to e.g. error out when contradictory settings are provided that are not
    // caught in the product config. This should be done via Validating Webhooks once available.
    #[snafu(display("invalid product specific configuration found: {reason}"))]
    InvalidProductSpecificConfiguration { reason: String },
}

/// This trait is used to compute configuration properties for products.
///
/// This needs to be implemented for every T in the [`crate::role_utils::CommonConfiguration`] struct
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
/// Check out `ser::to_hash_map` in the `product-config` library if you do need to convert a struct to a HashMap
/// in an easy way.
pub trait Configuration {
    type Configurable;

    // TODO: We need to pass in the existing config from parents to run validation checks and we should probably also pass in a "final" parameter or have another "finalize" method callback
    //  one for each role group, one for each role and one for all of it...
    fn compute_env(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>>;

    fn compute_cli(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>>;

    fn compute_files(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
        file: &str,
    ) -> Result<BTreeMap<String, Option<String>>>;
}

impl<T: Configuration + ?Sized> Configuration for Box<T> {
    type Configurable = T::Configurable;

    fn compute_env(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>> {
        T::compute_env(self, resource, role_name)
    }

    fn compute_cli(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<BTreeMap<String, Option<String>>> {
        T::compute_cli(self, resource, role_name)
    }

    fn compute_files(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
        file: &str,
    ) -> Result<BTreeMap<String, Option<String>>> {
        T::compute_files(self, resource, role_name, file)
    }
}

/// Type to sort config properties via kind (files, env, cli), via groups and via roles.
pub type RoleConfigByPropertyKind =
    HashMap<String, HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>>;

/// Type to sort config properties via kind (files, env, cli) and via groups.
pub type RoleGroupConfigByPropertyKind =
    HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>;

/// Type to sort config properties via kind (files, env, cli), via groups and via roles. This
/// is the validated output to be used in other operators. The difference to [`RoleConfigByPropertyKind`]
/// is that the properties BTreeMap does not contain any options.
pub type ValidatedRoleConfigByPropertyKind =
    HashMap<String, HashMap<String, HashMap<PropertyNameKind, BTreeMap<String, String>>>>;

/// Extracts the config properties keyed by PropertyKindName (files, cli, env) for a role and
/// role group.
///
/// # Arguments
/// - `role`        - The role name.
/// - `group`       - The role group name.
/// - `role_config` - The validated product configuration for each role and group.
pub fn config_for_role_and_group<'a>(
    role: &str,
    group: &str,
    role_config: &'a ValidatedRoleConfigByPropertyKind,
) -> Result<&'a HashMap<PropertyNameKind, BTreeMap<String, String>>> {
    let result = match role_config.get(role) {
        None => {
            return MissingRoleSnafu {
                role: role.to_string(),
            }
            .fail();
        }
        Some(group_config) => match group_config.get(group) {
            None => {
                return MissingRoleGroupSnafu {
                    role: role.to_string(),
                    role_group: group.to_string(),
                }
                .fail();
            }
            Some(config_by_property_kind) => config_by_property_kind,
        },
    };

    Ok(result)
}

/// Given the configuration parameters of all `roles` partition them by `PropertyNameKind` and
/// merge them with the role groups configuration parameters.
///
/// The output is a map keyed by the role names. The value is also a map keyed by role group names and
/// the values are the merged configuration properties "bucketed" by `PropertyNameKind`.
///
/// # Arguments
/// - `resource`  - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `roles`     - A map keyed by role names. The value is a tuple of a vector of `PropertyNameKind`
///                 like (Cli, Env or Files) and [`crate::role_utils::Role`] with a boxed [`Configuration`].
#[allow(clippy::type_complexity)]
pub fn transform_all_roles_to_config<T, U, ProductSpecificCommonConfig>(
    resource: &T::Configurable,
    roles: HashMap<
        String,
        (
            Vec<PropertyNameKind>,
            Role<T, U, ProductSpecificCommonConfig>,
        ),
    >,
) -> Result<RoleConfigByPropertyKind>
where
    T: Configuration,
    U: Default + JsonSchema + Serialize,
    ProductSpecificCommonConfig: Default + JsonSchema + Serialize,
{
    let mut result = HashMap::new();

    for (role_name, (property_name_kinds, role)) in &roles {
        let role_properties =
            transform_role_to_config(resource, role_name, role, property_name_kinds)?;
        result.insert(role_name.to_string(), role_properties);
    }

    Ok(result)
}

/// Validates a product configuration for all roles and role_groups. Requires a valid product config
/// and [`RoleConfigByPropertyKind`] which can be obtained via `transform_all_roles_to_config`.
///
/// # Arguments
/// - `version`            - The version of the product to be configured.
/// - `role_config`        - Collected information about all roles, role groups, required
///                          properties sorted by config files, CLI parameters and ENV variables.
/// - `product_config`     - The [`product_config::ProductConfigManager`] used to validate the provided
///                          user data.
/// - `ignore_warn`        - A switch to ignore product config warnings and continue with
///                          the value anyways. Not recommended!
/// - `ignore_err`         - A switch to ignore product config errors and continue with
///                          the value anyways. Not recommended!
pub fn validate_all_roles_and_groups_config(
    version: &str,
    role_config: &RoleConfigByPropertyKind,
    product_config: &ProductConfigManager,
    ignore_warn: bool,
    ignore_err: bool,
) -> Result<ValidatedRoleConfigByPropertyKind> {
    let mut result = HashMap::new();
    for (role, role_group) in role_config {
        result.insert(role.to_string(), HashMap::new());

        for (group, properties_by_kind) in role_group {
            result.get_mut(role).unwrap().insert(
                group.clone(),
                validate_role_and_group_config(
                    version,
                    role,
                    properties_by_kind,
                    product_config,
                    ignore_warn,
                    ignore_err,
                )?,
            );
        }
    }

    Ok(result)
}

/// Calculates and validates a product configuration for a role and group. Requires a valid
/// product config and existing [`RoleConfigByPropertyKind`] that can be obtained via
/// `transform_all_roles_to_config`.
///
/// # Arguments
/// - `role`               - The name of the role
/// - `version`            - The version of the product to be configured.
/// - `properties_by_kind` - Config properties sorted by PropertyKind
///                          and the resulting user configuration data. See [`RoleConfigByPropertyKind`].
/// - `product_config`     - The [`product_config::ProductConfigManager`] used to validate the provided
///                          user data.
/// - `ignore_warn`        - A switch to ignore product config warnings and continue with
///                          the value anyways. Not recommended!
/// - `ignore_err`         - A switch to ignore product config errors and continue with
///                          the value anyways. Not recommended!
fn validate_role_and_group_config(
    version: &str,
    role: &str,
    properties_by_kind: &HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>,
    product_config: &ProductConfigManager,
    ignore_warn: bool,
    ignore_err: bool,
) -> Result<HashMap<PropertyNameKind, BTreeMap<String, String>>> {
    let mut result = HashMap::new();

    for (property_name_kind, config) in properties_by_kind {
        let validation_result = product_config
            .get(
                version,
                role,
                property_name_kind,
                config.clone().into_iter().collect::<HashMap<_, _>>(),
            )
            .context(InvalidConfigurationSnafu)?;

        let validated_config =
            process_validation_result(&validation_result, ignore_warn, ignore_err)?;

        result.insert(property_name_kind.clone(), validated_config);
    }

    Ok(result)
}

/// This transforms the [`product_config::PropertyValidationResult`] back into a pure BTreeMap which can be used
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
fn process_validation_result(
    validation_result: &BTreeMap<String, PropertyValidationResult>,
    ignore_warn: bool,
    ignore_err: bool,
) -> Result<BTreeMap<String, String>> {
    let mut properties = BTreeMap::new();
    let mut collected_errors = Vec::new();

    for (key, result) in validation_result.iter() {
        match result {
            PropertyValidationResult::Default(value) => {
                debug!(
                    "Property [{}] is not explicitly set, will set and rely to the default instead ([{}])",
                    key, value
                );
                properties.insert(key.clone(), value.clone());
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
            PropertyValidationResult::Unknown(value) => {
                debug!(
                    "Property [{}] is unknown (no validation) and set to value [{}]",
                    key, value
                );
                properties.insert(key.clone(), value.clone());
            }
            PropertyValidationResult::Warn(value, err) => {
                warn!(
                    "Property [{}] is set to value [{}] which causes a warning, `ignore_warn` is {}: {:?}",
                    key, value, ignore_warn, err
                );
                if ignore_warn {
                    properties.insert(key.clone(), value.clone());
                }
            }
            PropertyValidationResult::Error(value, err) => {
                error!(
                    "Property [{}] is set to value [{}] which causes an error, `ignore_err` is {}: {:?}",
                    key, value, ignore_err, err
                );
                if ignore_err {
                    properties.insert(key.clone(), value.clone());
                } else {
                    collected_errors.push(err.clone());
                }
            }
        }
    }

    if !collected_errors.is_empty() {
        return ProductConfigErrorsSnafu { collected_errors }.fail();
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
/// group overrides -> role overrides -> group config -> role config (TODO: -> common_config)
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
fn transform_role_to_config<T, U, ProductSpecificCommonConfig>(
    resource: &T::Configurable,
    role_name: &str,
    role: &Role<T, U, ProductSpecificCommonConfig>,
    property_kinds: &[PropertyNameKind],
) -> Result<RoleGroupConfigByPropertyKind>
where
    T: Configuration,
    U: Default + JsonSchema + Serialize,
    ProductSpecificCommonConfig: Default + JsonSchema + Serialize,
{
    let mut result = HashMap::new();

    // Properties from the role have the lowest priority, so they are computed first...
    let role_properties = parse_role_config(resource, role_name, &role.config, property_kinds)?;
    let role_overrides = parse_role_overrides(&role.config, property_kinds)?;

    // for each role group ...
    for (role_group_name, role_group) in &role.role_groups {
        let mut role_group_properties_merged = role_properties.clone();

        // ... compute the group properties and merge them into role properties.
        let role_group_properties =
            parse_role_config(resource, role_name, &role_group.config, property_kinds)?;
        for (property_kind, properties) in role_group_properties {
            role_group_properties_merged
                .entry(property_kind)
                .or_default()
                .extend(properties);
        }

        // ... copy role overrides and merge them into `role_group_properties_merged`.
        for (property_kind, property_overrides) in role_overrides.clone() {
            role_group_properties_merged
                .entry(property_kind)
                .or_default()
                .extend(property_overrides);
        }

        // ... compute the role group overrides and merge them into `role_group_properties_merged`.
        let role_group_overrides = parse_role_overrides(&role_group.config, property_kinds)?;
        for (property_kind, property_overrides) in role_group_overrides {
            role_group_properties_merged
                .entry(property_kind)
                .or_default()
                .extend(property_overrides);
        }

        result.insert(role_group_name.clone(), role_group_properties_merged);
    }

    Ok(result)
}

/// Given a `config` object and the `property_kinds` vector, it uses the `Configuration::compute_*` methods
/// to partition the configuration properties by [`product_config::PropertyValidationResult`].
///
/// The output is a map where the configuration properties are keyed by [`product_config::PropertyValidationResult`].
///
/// # Arguments
/// - `resource`       - Not used directly. It's passed on to the `Configuration::compute_*` calls.
/// - `role_name`      - Not used directly but passed on to the `Configuration::compute_*` calls.
/// - `config`         - The configuration properties to partition.
/// - `property_kinds` - The "buckets" used to partition the configuration properties.
fn parse_role_config<T, ProductSpecificCommonConfig>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &CommonConfiguration<T, ProductSpecificCommonConfig>,
    property_kinds: &[PropertyNameKind],
) -> Result<HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();
    for property_kind in property_kinds {
        match property_kind {
            PropertyNameKind::File(file) => result.insert(
                property_kind.clone(),
                config.config.compute_files(resource, role_name, file)?,
            ),
            PropertyNameKind::Env => result.insert(
                property_kind.clone(),
                config.config.compute_env(resource, role_name)?,
            ),
            PropertyNameKind::Cli => result.insert(
                property_kind.clone(),
                config.config.compute_cli(resource, role_name)?,
            ),
        };
    }

    Ok(result)
}

fn parse_role_overrides<T, ProductSpecificCommonConfig>(
    config: &CommonConfiguration<T, ProductSpecificCommonConfig>,
    property_kinds: &[PropertyNameKind],
) -> Result<HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();
    for property_kind in property_kinds {
        match property_kind {
            PropertyNameKind::File(file) => {
                result.insert(property_kind.clone(), parse_file_overrides(config, file)?)
            }
            PropertyNameKind::Env => result.insert(
                property_kind.clone(),
                config
                    .env_overrides
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, Some(v)))
                    .collect(),
            ),
            PropertyNameKind::Cli => result.insert(
                property_kind.clone(),
                config
                    .cli_overrides
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, Some(v)))
                    .collect(),
            ),
        };
    }

    Ok(result)
}

fn parse_file_overrides<T, ProductSpecificCommonConfig>(
    config: &CommonConfiguration<T, ProductSpecificCommonConfig>,
    file: &str,
) -> Result<BTreeMap<String, Option<String>>>
where
    T: Configuration,
{
    let mut final_overrides: BTreeMap<String, Option<String>> = BTreeMap::new();

    // For Conf files only process overrides that match our file name
    if let Some(config) = config.config_overrides.get(file) {
        for (key, value) in config {
            final_overrides.insert(key.clone(), Some(value.clone()));
        }
    }

    Ok(final_overrides)
}

/// Extract the environment variables of a rolegroup config into a vector of EnvVars.
///
/// # Example
///
/// ```
/// use std::collections::{BTreeMap, HashMap};
///
/// use k8s_openapi::api::core::v1::EnvVar;
/// use product_config::types::PropertyNameKind;
/// use stackable_operator::product_config_utils::env_vars_from_rolegroup_config;
///
/// let rolegroup_config = [(
///     PropertyNameKind::Env,
///     [
///         ("VAR1".to_string(), "value 1".to_string()),
///         ("VAR2".to_string(), "value 2".to_string()),
///     ]
///     .into_iter()
///     .collect::<BTreeMap<_, _>>(),
/// )]
/// .into_iter()
/// .collect::<HashMap<_, _>>();
///
/// let expected_env_vars = vec![
///     EnvVar {
///         name: "VAR1".to_string(),
///         value: Some("value 1".to_string()),
///         value_from: None,
///     },
///     EnvVar {
///         name: "VAR2".to_string(),
///         value: Some("value 2".to_string()),
///         value_from: None,
///     },
/// ];
/// assert_eq!(
///     expected_env_vars,
///     env_vars_from_rolegroup_config(&rolegroup_config)
/// );
/// ```
pub fn env_vars_from_rolegroup_config(
    rolegroup_config: &HashMap<PropertyNameKind, BTreeMap<String, String>>,
) -> Vec<EnvVar> {
    env_vars_from(
        rolegroup_config
            .get(&PropertyNameKind::Env)
            .cloned()
            .unwrap_or_default(),
    )
}

/// Convert key-value structures into a vector of EnvVars.
///
/// # Example
///
/// ```
/// use k8s_openapi::api::core::v1::EnvVar;
/// use stackable_operator::{product_config_utils::env_vars_from, role_utils::CommonConfiguration};
///
/// let common_config = CommonConfiguration::<(), ()> {
///     env_overrides: [("VAR".to_string(), "value".to_string())]
///         .into_iter()
///         .collect(),
///     ..Default::default()
/// };
///
/// let env_vars = env_vars_from(common_config.env_overrides);
///
/// let expected_env_vars = vec![EnvVar {
///     name: "VAR".to_string(),
///     value: Some("value".to_string()),
///     value_from: None
/// }];
///
/// assert_eq!(expected_env_vars, env_vars);
/// ```
pub fn env_vars_from<I, K, V>(env_vars: I) -> Vec<EnvVar>
where
    I: IntoIterator<Item = (K, V)>,
    K: Clone + Into<String>,
    V: Clone + Into<String>,
{
    env_vars.into_iter().map(env_var_from_tuple).collect()
}

/// Convert a tuple of strings into an EnvVar
///
/// # Example
///
/// ```
/// use k8s_openapi::api::core::v1::EnvVar;
/// use stackable_operator::product_config_utils::env_var_from_tuple;
///
/// let tuple = ("VAR", "value");
///
/// let env_var = env_var_from_tuple(tuple);
///
/// let expected_env_var = EnvVar {
///     name: "VAR".to_string(),
///     value: Some("value".to_string()),
///     value_from: None,
/// };
/// assert_eq!(expected_env_var, env_var);
/// ```
pub fn env_var_from_tuple(entry: (impl Into<String>, impl Into<String>)) -> EnvVar {
    EnvVar {
        name: entry.0.into(),
        value: Some(entry.1.into()),
        value_from: None,
    }
}

/// Inserts or updates the EnvVars from `env_overrides` in `env_vars`.
///
/// The resulting vector is sorted by the EnvVar names.
///
/// # Example
///
/// ```
/// use stackable_operator::product_config_utils::{env_vars_from, insert_or_update_env_vars};
///
/// let env_vars = env_vars_from([
///     ("VAR1", "original value 1"),
///     ("VAR2", "original value 2")
/// ]);
/// let env_overrides = env_vars_from([
///     ("VAR2", "overriden value 2"),
///     ("VAR3", "new value 3")
/// ]);
///
/// let combined_env_vars = insert_or_update_env_vars(&env_vars, &env_overrides);
///
/// let expected_result = env_vars_from([
///     ("VAR1", "original value 1"),
///     ("VAR2", "overriden value 2"),
///     ("VAR3", "new value 3"),
/// ]);
///
/// assert_eq!(expected_result, combined_env_vars);
/// ```
pub fn insert_or_update_env_vars(env_vars: &[EnvVar], env_overrides: &[EnvVar]) -> Vec<EnvVar> {
    let mut combined = BTreeMap::new();

    for env_var in env_vars.iter().chain(env_overrides) {
        combined.insert(env_var.name.to_owned(), env_var.to_owned());
    }

    combined.into_values().collect()
}

#[cfg(test)]
mod tests {
    macro_rules! collection {
        // map-like
        ($($k:expr_2021 => $v:expr_2021),* $(,)?) => {
            [$(($k, $v),)*].into()
        };
        // set-like
        ($($v:expr_2021),* $(,)?) => {
            [$($v,)*].into()
        };
    }

    use std::{collections::HashMap, str::FromStr};

    use k8s_openapi::api::core::v1::PodTemplateSpec;
    use rstest::*;

    use super::*;
    use crate::role_utils::{GenericProductSpecificCommonConfig, Role, RoleGroup};

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

    #[derive(Clone, Default, Debug, PartialEq)]
    struct TestConfig {
        pub conf: Option<String>,
        pub env: Option<String>,
        pub cli: Option<String>,
    }

    #[derive(Clone, Default, Debug, PartialEq, JsonSchema, Serialize)]
    struct TestRoleConfig {}

    impl Configuration for TestConfig {
        type Configurable = String;

        fn compute_env(
            &self,
            _resource: &Self::Configurable,
            _role_name: &str,
        ) -> Result<BTreeMap<String, Option<String>>> {
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
        ) -> Result<BTreeMap<String, Option<String>>> {
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
        ) -> Result<BTreeMap<String, Option<String>>> {
            let mut result = BTreeMap::new();
            if let Some(conf) = &self.conf {
                result.insert("file".to_string(), Some(conf.to_string()));
            }
            Ok(result)
        }
    }

    fn build_test_config(conf: &str, env: &str, cli: &str) -> Option<Box<TestConfig>> {
        Some(Box::new(TestConfig {
            conf: Some(conf.to_string()),
            env: Some(env.to_string()),
            cli: Some(cli.to_string()),
        }))
    }

    fn build_common_config(
        test_config: Option<Box<TestConfig>>,
        config_overrides: Option<HashMap<String, HashMap<String, String>>>,
        env_overrides: Option<HashMap<String, String>>,
        cli_overrides: Option<BTreeMap<String, String>>,
    ) -> CommonConfiguration<Box<TestConfig>, GenericProductSpecificCommonConfig> {
        CommonConfiguration {
            config: test_config.unwrap_or_default(),
            config_overrides: config_overrides.unwrap_or_default(),
            env_overrides: env_overrides.unwrap_or_default(),
            cli_overrides: cli_overrides.unwrap_or_default(),
            pod_overrides: PodTemplateSpec::default(),
            product_specific_common_config: GenericProductSpecificCommonConfig::default(),
        }
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
    ) -> Role<Box<TestConfig>, TestRoleConfig> {
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
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (true, true, true, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI), None, None, None),
                }},
            },
            (true, true, false, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (true, true, false, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                }},
            },
            (true, false, true, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (true, false, true, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: CommonConfiguration::default(),
                }},
            },
            (true, false, false, true) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)
                    ),
                }},
            },
            (true, false, false, false) => Role {
                config: build_common_config(
                    build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                    None,
                    None,
                    None,
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: CommonConfiguration::default(),
                }},
            },
            (false, true, true, true) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (false, true, true, false) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                }},
            },
            (false, true, false, true) => Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (false, true, false, false) => Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                        None,
                        None,
                        None),
                }},
            },
            (false, false, true, true) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (false, false, true, false) => Role {
                config: build_common_config(
                    None,
                    build_config_override(file_name, ROLE_CONF_OVERRIDE),
                    build_env_override(ROLE_ENV_OVERRIDE),
                    build_cli_override(ROLE_CLI_OVERRIDE),
                ),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: CommonConfiguration::default(),
                }},
            },
            (false, false, false, true) => Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: build_common_config(
                        None,
                        build_config_override(file_name, GROUP_CONF_OVERRIDE),
                        build_env_override(GROUP_ENV_OVERRIDE),
                        build_cli_override(GROUP_CLI_OVERRIDE)),
                }},
            },
            (false, false, false, false) => Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {role_group => RoleGroup {
                    replicas: Some(1),
                    config: CommonConfiguration::default(),
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
    fn role_to_config(
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

        let config =
            transform_role_to_config(&String::new(), ROLE_GROUP, &role, &property_kinds).unwrap();

        assert_eq!(config, expected);
    }

    #[rstest]
    #[case(
        HashMap::from([
            ("env".to_string(), ROLE_ENV_OVERRIDE.to_string()),
        ]),
        HashMap::from([
            ("env".to_string(), GROUP_ENV_OVERRIDE.to_string()),
        ]),
        BTreeMap::from([
            ("cli".to_string(), ROLE_CLI_OVERRIDE.to_string()),
        ]),
        BTreeMap::from([
            ("cli".to_string(), GROUP_CLI_OVERRIDE.to_string()),
        ]),
        HashMap::from([
            ("file".to_string(), HashMap::from([
                ("file".to_string(), ROLE_CONF_OVERRIDE.to_string())
            ]))
        ]),
        HashMap::from([
            ("file".to_string(), HashMap::from([
                ("file".to_string(), GROUP_CONF_OVERRIDE.to_string())
            ]))
        ]),
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV_OVERRIDE.to_string()),
                },
                PropertyNameKind::Cli => collection ! {
                    "cli".to_string() => Some(GROUP_CLI_OVERRIDE.to_string()),
                },
                PropertyNameKind::File("file".to_string()) => collection ! {
                    "file".to_string() => Some(GROUP_CONF_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(
        HashMap::from([
            ("env".to_string(), ROLE_ENV_OVERRIDE.to_string()),
        ]),
        HashMap::from([]),
        BTreeMap::from([
            ("cli".to_string(), ROLE_CLI_OVERRIDE.to_string()),
        ]),
        BTreeMap::from([]),
        HashMap::from([
            ("file".to_string(), HashMap::from([
                ("file".to_string(), ROLE_CONF_OVERRIDE.to_string())
            ]))
        ]),
        HashMap::from([]),
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(ROLE_ENV_OVERRIDE.to_string()),
                },
                PropertyNameKind::Cli => collection ! {
                    "cli".to_string() => Some(ROLE_CLI_OVERRIDE.to_string()),
                },
                PropertyNameKind::File("file".to_string()) => collection ! {
                    "file".to_string() => Some(ROLE_CONF_OVERRIDE.to_string()),
                }
            }
        }
    )]
    #[case(
        HashMap::from([]),
        HashMap::from([]),
        BTreeMap::from([]),
        BTreeMap::from([]),
        HashMap::from([]),
        HashMap::from([]),
        collection ! {
            ROLE_GROUP.to_string() => collection ! {
                PropertyNameKind::Env => collection ! {
                    "env".to_string() => Some(GROUP_ENV.to_string()),
                },
                PropertyNameKind::Cli => collection ! {
                    "cli".to_string() => Some(GROUP_CLI.to_string()),
                },
                PropertyNameKind::File("file".to_string()) => collection ! {
                    "file".to_string() => Some(GROUP_CONFIG.to_string()),
                }
            }
        }
    )]
    fn order_in_transform_role_to_config(
        #[case] role_env_override: HashMap<String, String>,
        #[case] group_env_override: HashMap<String, String>,
        #[case] role_cli_override: BTreeMap<String, String>,
        #[case] group_cli_override: BTreeMap<String, String>,
        #[case] role_conf_override: HashMap<String, HashMap<String, String>>,
        #[case] group_conf_override: HashMap<String, HashMap<String, String>>,
        #[case] expected: HashMap<
            String,
            HashMap<PropertyNameKind, BTreeMap<String, Option<String>>>,
        >,
    ) {
        let role: Role<Box<TestConfig>, TestRoleConfig> = Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                Some(role_conf_override),
                Some(role_env_override),
                Some(role_cli_override),
            ),
            role_config: Default::default(),
            role_groups: collection! {"role_group".to_string() => RoleGroup {
                replicas: Some(1),
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    Some(group_conf_override),
                    Some(group_env_override),
                    Some(group_cli_override)),
            }},
        };

        let property_kinds = vec![
            PropertyNameKind::Env,
            PropertyNameKind::Cli,
            PropertyNameKind::File("file".to_string()),
        ];

        let config =
            transform_role_to_config(&String::new(), ROLE_GROUP, &role, &property_kinds).unwrap();

        assert_eq!(config, expected);
    }

    #[test]
    fn role_to_config_overrides() {
        let role_group = "role_group";
        let file_name = "foo.bar";
        let role = Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                // should override
                build_config_override(file_name, "file"),
                None,
                // should override
                build_cli_override("cli"),
            ),
            role_config: TestRoleConfig::default(),
            role_groups: collection! {role_group.to_string() => RoleGroup {
                replicas: Some(1),
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    // should override
                    build_config_override(file_name, "file"),
                    build_env_override(GROUP_ENV_OVERRIDE),
                    None),
            }},
        };

        let expected = collection! {
        role_group.to_string() =>
            collection!{
                PropertyNameKind::File(file_name.to_string()) =>
                    collection!(
                        "file".to_string() => Some("file".to_string())
                    ),
                PropertyNameKind::Env =>
                    collection!(
                        "env".to_string() => Some(GROUP_ENV.to_string()),
                        GROUP_ENV_OVERRIDE.to_string() => Some(GROUP_ENV_OVERRIDE.to_string())
                    ),
                PropertyNameKind::Cli =>
                    collection!(
                        "cli".to_string() => Some("cli".to_string()),
                    ),
            }
        };

        let property_kinds = vec![
            PropertyNameKind::File(file_name.to_string()),
            PropertyNameKind::Env,
            PropertyNameKind::Cli,
        ];

        let config =
            transform_role_to_config(&String::new(), ROLE_GROUP, &role, &property_kinds).unwrap();

        assert_eq!(config, expected);
    }

    #[test]
    fn all_roles_to_config() {
        let role_1 = "role_1";
        let role_2 = "role_2";
        let role_group_1 = "role_group_1";
        let role_group_2 = "role_group_2";
        let file_name = "foo.bar";

        #[allow(clippy::type_complexity)]
        let roles: HashMap<
            String,
            (Vec<PropertyNameKind>, Role<Box<TestConfig>, TestRoleConfig>),
        > = collection! {
            role_1.to_string() => (vec![PropertyNameKind::File(file_name.to_string()), PropertyNameKind::Env], Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                None,
                None,
                None,
            ),
            role_config: Default::default(),
            role_groups: collection! {role_group_1.to_string() => RoleGroup {
                replicas: Some(1),
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
            },
            role_group_2.to_string() => RoleGroup {
                replicas: Some(1),
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
            }}
        }),
            role_2.to_string() => (vec![PropertyNameKind::Cli], Role {
            config: build_common_config(
                build_test_config(ROLE_CONFIG, ROLE_ENV, ROLE_CLI),
                None,
                None,
                None,
            ),
            role_config: Default::default(),
            role_groups: collection! {role_group_1.to_string() => RoleGroup {
                replicas: Some(1),
                config: build_common_config(
                    build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                    None,
                    None,
                    None
                ),
            }},
        })
        };

        let expected: RoleConfigByPropertyKind = collection! {
        role_1.to_string() => collection!{
            role_group_1.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => Some(GROUP_ENV.to_string())
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "file".to_string() => Some(GROUP_CONFIG.to_string())
                }
            },
            role_group_2.to_string() => collection! {
                PropertyNameKind::Env => collection! {
                    "env".to_string() => Some(GROUP_ENV.to_string())
                },
                PropertyNameKind::File(file_name.to_string()) => collection! {
                    "file".to_string() => Some(GROUP_CONFIG.to_string())
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

        let all_config = transform_all_roles_to_config(&String::new(), roles).unwrap();

        assert_eq!(all_config, expected);
    }

    #[test]
    fn test_validate_all_roles_and_groups_config() {
        let role_1 = "role_1";
        let role_2 = "role_2";
        let role_group_1 = "role_group_1";
        let role_group_2 = "role_group_2";
        let file_name = "foo.bar";

        let pc_name = "pc_name";
        let pc_value = "pc_value";
        let pc_bad_version = "pc_bad_version";
        let pc_bad_version_value = "pc_bad_version_value";

        #[allow(clippy::type_complexity)]
        let roles: HashMap<
            String,
            (Vec<PropertyNameKind>, Role<Box<TestConfig>, TestRoleConfig>),
        > = collection! {
            role_1.to_string() => (vec![PropertyNameKind::File(file_name.to_string()), PropertyNameKind::Env], Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {
                    role_group_1.to_string() => RoleGroup {
                        replicas: Some(1),
                        config: build_common_config(
                            build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                            None,
                            None,
                            None
                        ),
                    }}
            }
            ),
            role_2.to_string() => (vec![PropertyNameKind::File(file_name.to_string())], Role {
                config: CommonConfiguration::default(),
                role_config: Default::default(),
                role_groups: collection! {
                    role_group_2.to_string() => RoleGroup {
                        replicas: Some(1),
                        config: build_common_config(
                            build_test_config(GROUP_CONFIG, GROUP_ENV, GROUP_CLI),
                            None,
                            None,
                            None
                        ),
                    }}
            }
            ),
        };

        let role_config = transform_all_roles_to_config(&String::new(), roles).unwrap();

        let config = &format!(
            "
            version: 0.1.0
            spec:
              units: []
            properties:
              - property:
                  propertyNames:
                    - name: \"{pc_name}\"
                      kind:
                        type: \"file\"
                        file: \"{file_name}\"
                  datatype:
                    type: \"string\"
                  recommendedValues:
                    - value: \"{pc_value}\"
                  roles:
                    - name: \"{role_1}\"
                      required: true
                    - name: \"{role_2}\"
                      required: true
                  asOfVersion: \"0.0.0\"
              - property:
                  propertyNames:
                    - name: \"{pc_bad_version}\"
                      kind:
                        type: \"file\"
                        file: \"{file_name}\"
                  datatype:
                    type: \"string\"
                  recommendedValues:
                    - value: \"{pc_bad_version_value}\"
                  roles:
                    - name: \"{role_1}\"
                      required: true
                  asOfVersion: \"0.5.0\"
            "
        );

        let product_config = ProductConfigManager::from_str(config).unwrap();

        let full_validated_config = validate_all_roles_and_groups_config(
            "0.1.0",
            &role_config,
            &product_config,
            false,
            false,
        )
        .unwrap();

        let expected: ValidatedRoleConfigByPropertyKind = collection! {
            role_1.to_string() => collection! {
              role_group_1.to_string() => collection! {
                PropertyNameKind::File(file_name.to_string()) => collection! {
                      "file".to_string() => GROUP_CONFIG.to_string(),
                      pc_name.to_string() => pc_value.to_string()
                },
                PropertyNameKind::Env => collection! {
                      "env".to_string() => GROUP_ENV.to_string()
                }
              }
            },
            role_2.to_string() => collection! {
              role_group_2.to_string() => collection! {
                PropertyNameKind::File(file_name.to_string()) => collection! {
                      "file".to_string() => GROUP_CONFIG.to_string(),
                      pc_name.to_string() => pc_value.to_string()
                },
              }
          }
        };
        assert_eq!(full_validated_config, expected);

        // test config_for_role_and_group
        let valid_config_for_role_and_group =
            config_for_role_and_group(role_1, role_group_1, &full_validated_config).unwrap();
        assert_eq!(
            expected.get(role_1).unwrap().get(role_group_1).unwrap(),
            valid_config_for_role_and_group
        );

        let config_for_wrong_role =
            config_for_role_and_group("wrong_role", "wrong_group", &full_validated_config);

        assert!(config_for_wrong_role.is_err());

        let config_for_role_and_wrong_group =
            config_for_role_and_group(role_1, "wrong_group", &full_validated_config);

        assert!(config_for_role_and_wrong_group.is_err());
    }
}
