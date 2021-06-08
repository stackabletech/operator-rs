use crate::role_utils::{CommonConfiguration, Role, RoleGroup};
use product_config::types::PropertyNameKind;
use product_config::PropertyValidationResult;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration found")]
    InvalidConfiguration,
}

pub trait Configuration {
    type Configurable;

    // TODO: Result needs file name?
    // TODO: Not sure if I need the role_name here
    // TODO: We need to pass in the existing config from parents to run validation checks and we should probably also pass in a "final" parameter or have another "finalize" method callback
    //  one for each role group, one for each role and one for all of it...
    fn compute_env(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<HashMap<String, String>, ConfigError>;

    fn compute_cli(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
    ) -> Result<HashMap<String, String>, ConfigError>;

    fn compute_properties(
        &self,
        resource: &Self::Configurable,
        role_name: &str,
        file: &str,
    ) -> Result<HashMap<String, String>, ConfigError>;

    // role_name -> Vec<PropertyNameKind>b
    // TODO: Not sure if we need/want this
    fn config_information() -> HashMap<String, (PropertyNameKind, String)>;
}

pub fn get_role_config<T>(
    role_name: &str,
    role: &Role<T>,
    property_kinds: &[PropertyNameKind],
    resource: &T::Configurable,
) -> HashMap<String, HashMap<PropertyNameKind, HashMap<String, String>>>
where
    T: Configuration,
{
    let mut role_properties = HashMap::new();

    // Each PropertyNameKind means either a config file, env properties or CLI argument.
    // These can be customized per role.
    // Each role we'll first make sure to process the role-wide configuration.
    // To do this we need to iterate over all the PropertyKinds for this role and first
    // compute the properties from the typed configuration and then make sure to apply the matching overrides.
    // Then we'll do the same again but iterate over each role group.
    // The result will be a Map<Role Name, Map<Role Group name, Map<Property Kind, Map<String, String>>>>
    for property_kind in property_kinds {
        match property_kind {
            PropertyNameKind::Conf(file) => role_properties.insert(
                property_kind.clone(),
                parse_conf_properties(resource, role_name, &role.config, file),
            ),
            PropertyNameKind::Env => role_properties.insert(
                property_kind.clone(),
                parse_env_properties(resource, role_name, &role.config),
            ),
            PropertyNameKind::Cli => role_properties.insert(
                property_kind.clone(),
                parse_cli_properties(resource, role_name, &role.config),
            ),
        };
    }

    let mut result = HashMap::new();
    // This is the second loop: This time over all role groups within a role
    for (rolegroup_name, role_group) in &role.role_groups {
        let mut rolegroup_properties = HashMap::new();

        for property_kind in property_kinds {
            match property_kind {
                PropertyNameKind::Conf(file) => rolegroup_properties.insert(
                    property_kind.clone(),
                    parse_conf_properties(resource, role_name, &role_group.config, file),
                ),
                PropertyNameKind::Env => rolegroup_properties.insert(
                    property_kind.clone(),
                    parse_env_properties(resource, role_name, &role_group.config),
                ),
                PropertyNameKind::Cli => rolegroup_properties.insert(
                    property_kind.clone(),
                    parse_cli_properties(resource, role_name, &role_group.config),
                ),
            };
        }

        let mut foo = role_properties.clone();

        for (property_kind, properties) in rolegroup_properties {
            foo.entry(property_kind).or_default().extend(properties);
        }

        result.insert(rolegroup_name.clone(), foo);
    }

    result
}

pub fn get_all_config<T>(
    resource: &T::Configurable,
    // HashMap<Role Name, Vec<...>>
    role_information: HashMap<String, Vec<PropertyNameKind>>,
    // HashMap<Role name, (Role, Vec<RoleGroup>)>
    roles: HashMap<String, (Role<T>, HashMap<String, RoleGroup<T>>)>,
) -> HashMap<String, HashMap<String, HashMap<PropertyNameKind, HashMap<String, String>>>>
where
    T: Configuration,
{
    let mut result = HashMap::new();

    for (role_name, (role, _role_groups)) in roles {
        let role_properties = get_role_config(
            &role_name,
            &role,
            role_information.get(&role_name).unwrap(),
            resource,
        );
        result.insert(role_name, role_properties);
    }

    result
}

fn parse_cli_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> HashMap<String, String>
where
    T: Configuration,
{
    let mut final_properties = HashMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref config),
        ..
    }) = config
    {
        final_properties = config.compute_cli(resource, &role_name).unwrap();
    }

    // ...followed by config_overrides from the role
    if let Some(CommonConfiguration {
        cli_overrides: Some(ref config),
        ..
    }) = config
    {
        for (key, value) in config {
            final_properties.insert(key.clone(), value.clone().unwrap_or_default());
        }
    }

    final_properties
}

fn parse_env_properties<T>(
    resource: &<T as Configuration>::Configurable,
    role_name: &str,
    config: &Option<CommonConfiguration<T>>,
) -> HashMap<String, String>
where
    T: Configuration,
{
    let mut final_properties = HashMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref config),
        ..
    }) = config
    {
        final_properties = config.compute_env(resource, &role_name).unwrap();
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
) -> HashMap<String, String>
where
    T: Configuration,
{
    let mut final_properties = HashMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(CommonConfiguration {
        config: Some(ref config),
        ..
    }) = config
    {
        final_properties = config
            .compute_properties(resource, &role_name, file)
            .unwrap();
    }

    // ...followed by config_overrides from the role
    if let Some(CommonConfiguration {
        config_overrides: Some(ref config),
        ..
    }) = config
    {
        // For Conf files only process overrides that match our file name
        if let Some(config) = config.get(file) {
            for (key, value) in config {
                final_properties.insert(key.clone(), value.clone());
            }
        }
    }

    final_properties
}

// TODO: boolean flags suck, move ignore_warn to be a flag
pub fn process_validation_result(
    validation_result: &HashMap<String, PropertyValidationResult>,
    ignore_warn: bool,
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
            PropertyValidationResult::Error(err) => {
                error!(
                    "Property [{}] causes a validation error, will not set: {:?}",
                    key, err
                );
                //TODO: Return error
            }
        }
    }
    properties
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
            resource: &Self::Configurable,
            role_name: &str,
        ) -> Result<HashMap<String, String>, ConfigError> {
            let mut result = HashMap::new();
            if let Some(env) = &self.env {
                result.insert("env".to_string(), env.to_string());
            }
            Ok(result)
        }

        fn compute_cli(
            &self,
            resource: &Self::Configurable,
            role_name: &str,
        ) -> Result<HashMap<String, String>, ConfigError> {
            let mut result = HashMap::new();
            if let Some(cli) = &self.cli {
                result.insert("cli".to_string(), cli.to_string());
            }
            Ok(result)
        }

        fn compute_properties(
            &self,
            resource: &Self::Configurable,
            role_name: &str,
            file: &str,
        ) -> Result<HashMap<String, String>, ConfigError> {
            let mut result = HashMap::new();
            if let Some(conf) = &self.conf {
                result.insert("conf".to_string(), conf.to_string());
            }
            Ok(result)
        }

        fn config_information() -> HashMap<String, (PropertyNameKind, String)> {
            todo!()
        }
    }

    fn build_role_and_group(
        role_config: bool,
        group_config: bool,
        role_overrides: bool,
        group_overrides: bool,
    ) -> Role<TestConfig> {
        //let role = "role".to_string();
        let role_group = "role_group".to_string();

        let mut role = Role {
            config: None,
            role_groups: HashMap::new(),
        };

        if role_config {
            role.config = Some(CommonConfiguration {
                config: Some(TestConfig {
                    conf: Some("role_conf".to_string()),
                    env: Some("role_env".to_string()),
                    cli: Some("role_cli".to_string()),
                }),
                config_overrides: None,
                env_overrides: None,
                cli_overrides: None,
            })
        }

        if group_config {
            role.role_groups.insert(
                role_group.clone(),
                RoleGroup {
                    instances: 1,
                    config: Some(CommonConfiguration {
                        config: Some(TestConfig {
                            conf: Some("group_conf".to_string()),
                            env: Some("group_env".to_string()),
                            cli: Some("group_cli".to_string()),
                        }),
                        config_overrides: None,
                        env_overrides: if group_overrides {
                            Some(collection! {"group_override".to_string() => "env".to_string() })
                        } else {
                            None
                        },
                        cli_overrides: None,
                    }),
                    selector: None,
                },
            );
        }

        if role_overrides {
            if let Some(conf) = &mut role.config {
                conf.env_overrides =
                    Some(collection! {"role_override".to_string() => "env".to_string() });
            }
        }

        role
    }

    #[rstest]
    #[case(
        true,
        false,
        false,
        false,
        collection!{
            "role".to_string() =>
            collection!{
                PropertyNameKind::Env =>
                collection!{
                    "env".to_string() => "role_env".to_string(),
                }
            }
        }
    )]
    #[trace]
    fn test_get_role_config(
        #[case] role_config: bool,
        #[case] group_config: bool,
        #[case] role_overrides: bool,
        #[case] group_overrides: bool,
        #[case] expected: HashMap<String, HashMap<PropertyNameKind, HashMap<String, String>>>,
    ) {
        let role_name = "role";
        let role = build_role_and_group(role_config, group_config, role_overrides, group_overrides);

        let property_kinds = vec![PropertyNameKind::Env];

        let config = get_role_config(role_name, &role, &property_kinds, &String::new());

        println!("{:?}", role);
        println!("{:?}", expected);
        println!("{:?}", config);

        assert_eq!(config, expected);
    }

    #[test]
    fn test_role_without_config() {}

    #[test]
    fn test_role_with_config_without_group_config() {}

    #[test]
    fn test_role_with_config_with_group_config() {}

    #[test]
    fn test_role_with_config_and_config_override() {}

    #[test]
    fn test_role_with_config_and_env_override() {}

    #[test]
    fn test_role_with_config_and_cli_override() {}

    #[test]
    fn test_role_with_config_and_group_with_config_and_config_override() {}

    #[test]
    fn test_role_without_config_and_group_with_config_and_config_override() {}

    #[test]
    fn test_role_without_config_and_group_without_config_and_config_override() {}
}
