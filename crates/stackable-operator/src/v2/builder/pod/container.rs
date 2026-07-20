use std::{
    collections::{BTreeMap, btree_map},
    str::FromStr,
};

use snafu::{ResultExt, Snafu};
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::{
    attributed_string_type,
    builder::pod::container::{ContainerBuilder, FieldPathEnvVar},
    k8s_openapi::api::core::v1::{
        ConfigMapKeySelector, EnvVar, EnvVarSource, ObjectFieldSelector, SecretKeySelector,
    },
    v2::types::kubernetes::{ConfigMapKey, ConfigMapName, ContainerName, SecretKey, SecretName},
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display(
        "invalid environment variable name: a valid environment variable name must not be empty \
        and must consist only of printable ASCII characters other than '='"
    ))]
    ParseEnvVarName {
        source: crate::v2::macros::attributed_string_type::Error,
    },
}

/// Infallible variant of [`crate::builder::pod::container::ContainerBuilder::new`]
pub fn new_container_builder(container_name: &ContainerName) -> ContainerBuilder {
    ContainerBuilder::new(container_name.as_ref()).expect("should be a valid container name")
}

attributed_string_type! {
    EnvVarName,
    "The name of an environment variable",
    "MY_ENV_VAR",
    (min_length = 1),
    (regex = "^[ -<>-~]+$")
    // The maximum length of environment variable names seems not to be restricted.
}

/// A set of [`EnvVar`]s
///
/// The environment variable names in the set are unique.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnvVarSet(BTreeMap<EnvVarName, EnvVar>);

impl EnvVarSet {
    /// Creates an empty [`EnvVarSet`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the [`EnvVar`] with the given name
    pub fn get(&self, env_var_name: &EnvVarName) -> Option<&EnvVar> {
        self.0.get(env_var_name)
    }

    /// Moves all [`EnvVar`]s from the given set into this one.
    ///
    /// [`EnvVar`]s with the same name are overridden.
    pub fn merge(mut self, mut env_var_set: Self) -> Self {
        self.0.append(&mut env_var_set.0);

        self
    }

    /// Adds the given [`EnvVar`] to this set
    ///
    /// An [`EnvVar`] with the same name is overridden.
    pub fn with_env_var(mut self, env_var: EnvVar) -> Result<Self> {
        self.0.insert(
            EnvVarName::from_str(&env_var.name).context(ParseEnvVarNameSnafu)?,
            env_var,
        );

        Ok(self)
    }

    /// Adds the given [`EnvVar`]s to this set
    ///
    /// [`EnvVar`]s with the same name are overridden.
    pub fn with_values<I, V>(self, env_vars: I) -> Self
    where
        I: IntoIterator<Item = (EnvVarName, V)>,
        V: Into<String>,
    {
        env_vars
            .into_iter()
            .fold(self, |extended_env_vars, (name, value)| {
                extended_env_vars.with_value(&name, value)
            })
    }

    /// Adds an environment variable with the given name and string value to this set
    ///
    /// An [`EnvVar`] with the same name is overridden.
    pub fn with_value(mut self, name: &EnvVarName, value: impl Into<String>) -> Self {
        self.0.insert(
            name.clone(),
            EnvVar {
                name: name.to_string(),
                value: Some(value.into()),
                value_from: None,
            },
        );

        self
    }

    /// Adds an environment variable with the given name and field path to this set
    ///
    /// An [`EnvVar`] with the same name is overridden.
    pub fn with_field_path(mut self, name: &EnvVarName, field_path: &FieldPathEnvVar) -> Self {
        self.0.insert(
            name.clone(),
            EnvVar {
                name: name.to_string(),
                value: None,
                value_from: Some(EnvVarSource {
                    field_ref: Some(ObjectFieldSelector {
                        field_path: field_path.to_string(),
                        ..ObjectFieldSelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            },
        );

        self
    }

    /// Adds an environment variable with the given ConfigMap key reference to this set
    ///
    /// An [`EnvVar`] with the same name is overridden.
    pub fn with_config_map_key_ref(
        mut self,
        name: &EnvVarName,
        config_map_name: &ConfigMapName,
        config_map_key: &ConfigMapKey,
    ) -> Self {
        self.0.insert(
            name.clone(),
            EnvVar {
                name: name.to_string(),
                value: None,
                value_from: Some(EnvVarSource {
                    config_map_key_ref: Some(ConfigMapKeySelector {
                        key: config_map_key.to_string(),
                        name: config_map_name.to_string(),
                        ..ConfigMapKeySelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            },
        );

        self
    }

    /// Adds an environment variable with the given Secret key reference to this set
    ///
    /// An [`EnvVar`] with the same name is overridden.
    pub fn with_secret_key_ref(
        mut self,
        name: &EnvVarName,
        secret_name: &SecretName,
        secret_key: &SecretKey,
    ) -> Self {
        self.0.insert(
            name.clone(),
            EnvVar {
                name: name.to_string(),
                value: None,
                value_from: Some(EnvVarSource {
                    secret_key_ref: Some(SecretKeySelector {
                        key: secret_key.to_string(),
                        name: secret_name.to_string(),
                        ..SecretKeySelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            },
        );

        self
    }
}

impl From<EnvVarSet> for Vec<EnvVar> {
    fn from(value: EnvVarSet) -> Self {
        value.0.values().cloned().collect()
    }
}

impl IntoIterator for EnvVarSet {
    type IntoIter = btree_map::IntoValues<EnvVarName, Self::Item>;
    type Item = EnvVar;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_values()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{EnvVarName, EnvVarSet};
    use crate::{
        builder::pod::container::FieldPathEnvVar,
        k8s_openapi::api::core::v1::{
            ConfigMapKeySelector, EnvVar, EnvVarSource, ObjectFieldSelector, SecretKeySelector,
        },
        v2::{
            builder::pod::container::new_container_builder,
            types::kubernetes::{
                ConfigMapKey, ConfigMapName, ContainerName, SecretKey, SecretName,
            },
        },
    };

    #[test]
    fn test_envvarname_fromstr() {
        // actually accepted by Kubernetes
        assert!(EnvVarName::from_str(" !\"#$%&'()*+,-./0123456789:;<>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~").is_ok());

        // empty string
        assert!(EnvVarName::from_str("").is_err());
        // non-printable ASCII characters
        assert!(EnvVarName::from_str("\n").is_err());
        assert!(EnvVarName::from_str("€").is_err());
        // equals sign
        assert!(EnvVarName::from_str("=").is_err());
    }

    #[test]
    fn test_new_container_builder() {
        // Test that the function does not panic
        new_container_builder(&ContainerName::from_str_unsafe("valid-container-name"));
    }

    #[test]
    fn test_envvarname_format() {
        assert_eq!(
            "TEST".to_owned(),
            format!("{}", EnvVarName::from_str_unsafe("TEST"))
        );
    }

    #[test]
    fn test_envvarset_merge() {
        let env_var_set1 = EnvVarSet::new().with_values([
            (
                EnvVarName::from_str_unsafe("ENV1"),
                "value1 from env_var_set1",
            ),
            (
                EnvVarName::from_str_unsafe("ENV2"),
                "value2 from env_var_set1",
            ),
            (
                EnvVarName::from_str_unsafe("ENV3"),
                "value3 from env_var_set1",
            ),
        ]);
        let env_var_set2 = EnvVarSet::new()
            .with_value(
                &EnvVarName::from_str_unsafe("ENV2"),
                "value2 from env_var_set2",
            )
            .with_field_path(&EnvVarName::from_str_unsafe("ENV3"), &FieldPathEnvVar::Name)
            .with_value(
                &EnvVarName::from_str_unsafe("ENV4"),
                "value4 from env_var_set2",
            );

        let merged_env_var_set = env_var_set1.merge(env_var_set2);

        assert_eq!(
            vec![
                EnvVar {
                    name: "ENV1".to_owned(),
                    value: Some("value1 from env_var_set1".to_owned()),
                    value_from: None
                },
                EnvVar {
                    name: "ENV2".to_owned(),
                    value: Some("value2 from env_var_set2".to_owned()),
                    value_from: None
                },
                EnvVar {
                    name: "ENV3".to_owned(),
                    value: None,
                    value_from: Some(EnvVarSource {
                        field_ref: Some(ObjectFieldSelector {
                            field_path: "metadata.name".to_owned(),
                            ..ObjectFieldSelector::default()
                        }),
                        ..EnvVarSource::default()
                    }),
                },
                EnvVar {
                    name: "ENV4".to_owned(),
                    value: Some("value4 from env_var_set2".to_owned()),
                    value_from: None
                }
            ],
            Vec::from(merged_env_var_set)
        );
    }

    #[test]
    fn test_envvarset_with_env_var() {
        let env_var_set = EnvVarSet::new()
            .with_env_var(EnvVar {
                name: "ENV".to_owned(),
                value: Some("value".to_owned()),
                value_from: None,
            })
            .expect("should be a valid EnvVar");

        assert_eq!(
            Some(&EnvVar {
                name: "ENV".to_owned(),
                value: Some("value".to_owned()),
                value_from: None
            }),
            env_var_set.get(&EnvVarName::from_str_unsafe("ENV"))
        );
    }

    #[test]
    fn test_envvarset_with_values() {
        let env_var_set = EnvVarSet::new().with_values([
            (EnvVarName::from_str_unsafe("ENV1"), "value1"),
            (EnvVarName::from_str_unsafe("ENV2"), "value2"),
        ]);

        assert_eq!(
            vec![
                EnvVar {
                    name: "ENV1".to_owned(),
                    value: Some("value1".to_owned()),
                    value_from: None
                },
                EnvVar {
                    name: "ENV2".to_owned(),
                    value: Some("value2".to_owned()),
                    value_from: None
                }
            ],
            Vec::from(env_var_set)
        );
    }

    #[test]
    fn test_envvarset_with_value() {
        let env_var_set = EnvVarSet::new().with_value(&EnvVarName::from_str_unsafe("ENV"), "value");

        assert_eq!(
            Some(&EnvVar {
                name: "ENV".to_owned(),
                value: Some("value".to_owned()),
                value_from: None
            }),
            env_var_set.get(&EnvVarName::from_str_unsafe("ENV"))
        );
    }

    #[test]
    fn test_envvarset_with_field_path() {
        let env_var_set = EnvVarSet::new()
            .with_field_path(&EnvVarName::from_str_unsafe("ENV"), &FieldPathEnvVar::Name);

        assert_eq!(
            Some(&EnvVar {
                name: "ENV".to_owned(),
                value: None,
                value_from: Some(EnvVarSource {
                    field_ref: Some(ObjectFieldSelector {
                        field_path: "metadata.name".to_owned(),
                        ..ObjectFieldSelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            }),
            env_var_set.get(&EnvVarName::from_str_unsafe("ENV"))
        );
    }

    #[test]
    fn test_envvarset_with_config_map_key_ref() {
        let env_var_set = EnvVarSet::new().with_config_map_key_ref(
            &EnvVarName::from_str_unsafe("ENV"),
            &ConfigMapName::from_str_unsafe("config-map"),
            &ConfigMapKey::from_str_unsafe("key"),
        );

        assert_eq!(
            Some(&EnvVar {
                name: "ENV".to_owned(),
                value: None,
                value_from: Some(EnvVarSource {
                    config_map_key_ref: Some(ConfigMapKeySelector {
                        key: "key".to_owned(),
                        name: "config-map".to_owned(),
                        ..ConfigMapKeySelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            }),
            env_var_set.get(&EnvVarName::from_str_unsafe("ENV"))
        );
    }

    #[test]
    fn test_envvarset_with_secret_key_ref() {
        let env_var_set = EnvVarSet::new().with_secret_key_ref(
            &EnvVarName::from_str_unsafe("ENV"),
            &SecretName::from_str_unsafe("secret"),
            &SecretKey::from_str_unsafe("key"),
        );

        assert_eq!(
            Some(&EnvVar {
                name: "ENV".to_owned(),
                value: None,
                value_from: Some(EnvVarSource {
                    secret_key_ref: Some(SecretKeySelector {
                        key: "key".to_owned(),
                        name: "secret".to_owned(),
                        ..SecretKeySelector::default()
                    }),
                    ..EnvVarSource::default()
                }),
            }),
            env_var_set.get(&EnvVarName::from_str_unsafe("ENV"))
        );
    }
}
