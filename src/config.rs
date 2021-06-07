use crate::role_utils::{CommonConfiguration, Role, RoleGroup};
use product_config::types::PropertyNameKind;
use product_config::PropertyValidationResult;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, warn};
use crate::reconcile::ReconcileResult;

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

pub fn get_all_config<T>(
    resource: &T::Configurable,
    role_information: HashMap<String, Vec<PropertyNameKind>>,
    roles: HashMap<String, (Role<T>, Vec<RoleGroup<T>>)>,
) where
    T: Configuration,
{
    for (role_name, (role, role_groups)) in roles {
        let property_kinds = match role_information.get(&role_name) {
            None => todo!("TODO, error"),
            Some(info) => info,
        };

        let mut role_properties: HashMap<PropertyNameKind, HashMap<String, String>> =
            HashMap::new();

        // Each PropertyNameKind means either a config file, env properties or CLI argument.
        // These can be customized per role.
        // Each role we'll first make sure to process the role-wide configuration.
        // To do this we need to iterate over all the PropertyKinds for this role and first
        // compute the properties from the typed configuration and then make sure to apply the matching overrides.
        // Then we'll do the same again but iterate over each role group.
        // The result will be a Map<Role Name, Map<Role Group name, Map<Property Kind, Map<String, String>>>>
        for property_kind in property_kinds {
            match property_kind {
                PropertyNameKind::Conf(file) => {
                    // Properties from the role have the lowest priority, so they are computed and added first...
                    if let Some(CommonConfiguration {
                        config: Some(ref config),
                        ..
                    }) = role.config
                    {
                        role_properties
                            .entry(property_kind.clone())
                            .or_default()
                            .extend(
                                config
                                    .compute_properties(resource, &role_name, file)
                                    .unwrap(),
                            );
                    }

                    // ...followed by config_overrides from the role
                    if let Some(CommonConfiguration {
                        config_overrides: Some(ref config),
                        ..
                    }) = role.config
                    {
                        // For Conf files only process overrides that match our file name
                        if let Some(config) = config.get(file) {
                            let mut override_map = HashMap::new();
                            for (key, value) in config {
                                override_map.insert(key.clone(), value.clone());
                            }
                            role_properties
                                .entry(property_kind.clone())
                                .or_default()
                                .extend(override_map);
                        }
                    }
                }
                PropertyNameKind::Env => {
                    // Properties from the role have the lowest priority, so they are computed and added first...
                    if let Some(CommonConfiguration {
                        config: Some(ref config),
                        ..
                    }) = role.config
                    {
                        role_properties
                            .entry(property_kind.clone())
                            .or_default()
                            .extend(config.compute_env(resource, &role_name).unwrap());
                    }

                    // ...followed by config_overrides from the role
                    if let Some(CommonConfiguration {
                        env_overrides: Some(ref config),
                        ..
                    }) = role.config
                    {
                        let mut override_map = HashMap::new();
                        for (key, value) in config {
                            override_map.insert(key.clone(), value.clone());
                        }
                        role_properties
                            .entry(property_kind.clone())
                            .or_default()
                            .extend(override_map);
                    }
                }
                PropertyNameKind::Cli => {
                    // Properties from the role have the lowest priority, so they are computed and added first...
                    if let Some(CommonConfiguration {
                        config: Some(ref config),
                        ..
                    }) = role.config
                    {
                        role_properties
                            .entry(property_kind.clone())
                            .or_default()
                            .extend(config.compute_cli(resource, &role_name).unwrap());
                    }

                    // ...followed by config_overrides from the role
                    if let Some(CommonConfiguration {
                        cli_overrides: Some(ref config),
                        ..
                    }) = role.config
                    {
                        // TODO: This is dirty, not sure how to handle CLI stuff yet
                        //  Accepting just a single string makes it easier because we don't have to deal with
                        //  how to pass things on especially with arguments that have no parameter ("--foo")
                        let mut override_map = HashMap::new();
                        for key in config {
                            override_map.insert(key.clone(), "".to_string());
                        }
                        role_properties
                            .entry(property_kind.clone())
                            .or_default()
                            .extend(override_map);
                    }
                }
            }
        }

        // This is the second loop: This time over all role groups within a role
        for role_group in role_groups {
            let rolegroup_properties = HashMap::new();

            for property_kind in property_kinds {
                match property_kind {
                    PropertyNameKind::Conf(file) => {
                        // Properties from the role have the lowest priority, so they are computed and added first...
                        if let Some(CommonConfiguration {
                            config: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            rolegroup.entry(property_kind.clone()).or_default().extend(
                                config
                                    .compute_properties(resource, &role_name, file)
                                    .unwrap(),
                            );
                        }

                        // ...followed by config_overrides from the role
                        if let Some(CommonConfiguration {
                            config_overrides: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            // For Conf files only process overrides that match our file name
                            if let Some(config) = config.get(file) {
                                let mut override_map = HashMap::new();
                                for (key, value) in config {
                                    override_map.insert(key.clone(), value.clone());
                                }
                                role_properties
                                    .entry(property_kind.clone())
                                    .or_default()
                                    .extend(override_map);
                            }
                        }
                    }
                    PropertyNameKind::Env => {
                        // Properties from the role have the lowest priority, so they are computed and added first...
                        if let Some(CommonConfiguration {
                            config: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            role_properties
                                .entry(property_kind.clone())
                                .or_default()
                                .extend(config.compute_env(resource, &role_name).unwrap());
                        }

                        // ...followed by config_overrides from the role
                        if let Some(CommonConfiguration {
                            env_overrides: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            let mut override_map = HashMap::new();
                            for (key, value) in config {
                                override_map.insert(key.clone(), value.clone());
                            }
                            role_properties
                                .entry(property_kind.clone())
                                .or_default()
                                .extend(override_map);
                        }
                    }
                    PropertyNameKind::Cli => {
                        // Properties from the role have the lowest priority, so they are computed and added first...
                        if let Some(CommonConfiguration {
                            config: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            role_properties
                                .entry(property_kind.clone())
                                .or_default()
                                .extend(config.compute_cli(resource, &role_name).unwrap());
                        }

                        // ...followed by config_overrides from the role
                        if let Some(CommonConfiguration {
                            cli_overrides: Some(ref config),
                            ..
                        }) = role_group.config
                        {
                            // TODO: This is dirty, not sure how to handle CLI stuff yet
                            //  Accepting just a single string makes it easier because we don't have to deal with
                            //  how to pass things on especially with arguments that have no parameter ("--foo")
                            let mut override_map = HashMap::new();
                            for key in config {
                                override_map.insert(key.clone(), "".to_string());
                            }
                            role_properties
                                .entry(property_kind.clone())
                                .or_default()
                                .extend(override_map);
                        }
                    }
                }
            }
        }
    }
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

pub fn reconcile() -> ReconcileResult {

    let changed = reconcile_pod1(...);
    if changed {
        return ReconcileResult::Requeue
    }

    let changed = reconcile_configmap1(...);
    if changed {
        return ReconcileResult::Requeue
    }

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
