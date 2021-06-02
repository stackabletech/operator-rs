use crate::role_utils::Role;
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

    fn compute_properties(
        &self,
        resource: &Self::Configurable,
    ) -> Result<HashMap<String, String>, ConfigError>;
}

pub fn get_config<T>(
    resource: &T::Configurable,
    role: &Role<T>,
    role_group: &str,
) -> Result<HashMap<String, String>, ConfigError>
where
    T: Configuration,
{
    let role_group = match role.role_groups.get(role_group) {
        Some(role_group) => role_group,
        None => panic!("TODO, return error"),
    };

    let mut final_properties = HashMap::new();

    // Properties from the role have the lowest priority, so they are computed and added first...
    if let Some(ref config) = role.config {
        final_properties = config.compute_properties(resource)?;
    }

    // ...followed by config_overrides from the role
    if let Some(ref config) = role.config_overrides {
        for (key, value) in config {
            final_properties.insert(key.clone(), value.clone());
        }
    }

    // ...and now we need to check the config from the role group...
    if let Some(ref config) = role_group.config {
        final_properties.extend(config.compute_properties(resource)?);
    }

    // ...followed by the role group specific overrides.
    if let Some(ref config) = role_group.config_overrides {
        for (key, value) in config {
            final_properties.insert(key.clone(), value.clone());
        }
    }

    Ok(final_properties)
}

pub fn process_validation_result(
    validation_result: &[(String, PropertyValidationResult)],
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
    use super::*;
    use crate::role_utils::{Role, RoleGroup};
    use std::collections::HashMap;

    struct TestConfig {
        pub return_error: bool,
    }

    impl Configuration for TestConfig {
        type Configurable = String;

        fn compute_properties(
            &self,
            resource: &Self::Configurable,
        ) -> Result<HashMap<String, String>, ConfigError> {
            println!("Resource: {}", resource);
            if self.return_error {
                Err(ConfigError::InvalidConfiguration)
            } else {
                let mut result = HashMap::new();
                result.insert("test_property".to_string(), "true".to_string());
                Ok(result)
            }
        }
    }

    #[test]
    fn test() {
        let mut role_groups = HashMap::new();
        role_groups.insert(
            "foobar".to_string(),
            RoleGroup {
                instances: 1,
                config: None,
                config_overrides: None,
                selector: None,
            },
        );

        let role = Role {
            config: Some(TestConfig {
                return_error: false,
            }),
            config_overrides: None,
            role_groups,
        };

        let config = get_config(&"foo".to_string(), &role, "foobar").unwrap();
    }
}
