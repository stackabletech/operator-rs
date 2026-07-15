use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::LazyLock,
    vec,
};

use regex::Regex;
use snafu::{ResultExt, Snafu};
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::{
    attributed_string_type,
    builder::pod::container::{ContainerBuilder, FieldPathEnvVar},
    k8s_openapi::api::core::v1::{ConfigMapKeySelector, EnvVar, EnvVarSource, ObjectFieldSelector},
    v2::types::kubernetes::{ConfigMapKey, ConfigMapName, ContainerName},
};

/// Pattern for an escaped dollar sign, e.g. `$$`
static ESCAPED_DOLLAR_SIGN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\$").expect("should be a valid regular expression"));

/// Pattern for a referenced environment variable, e.g. `$(ENV_VAR)`
static REFERENCED_ENV_VARS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\(([^\)]+)\)").expect("should be a valid regular expression"));

/// Maximum recursion depth until references in environment variables are followed
const ENV_VAR_DEPENDENCY_RESOLVER_MAX_RECURSION_DEPTH: usize = 10;

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
}

impl From<EnvVarSet> for Vec<EnvVar> {
    fn from(value: EnvVarSet) -> Self {
        let env_var_closure =
            EnvVarDependencyResolver::new(&value, ENV_VAR_DEPENDENCY_RESOLVER_MAX_RECURSION_DEPTH);

        let mut vec: Self = value.0.values().cloned().collect();
        vec.sort_by_key(|env_var| env_var_closure.sort_key(env_var));
        vec
    }
}

impl IntoIterator for EnvVarSet {
    type IntoIter = vec::IntoIter<Self::Item>;
    type Item = EnvVar;

    fn into_iter(self) -> Self::IntoIter {
        Vec::from(self).into_iter()
    }
}

/// Resolves dependencies between environment variables and provides sort keys which take these
/// dependencies into account
pub struct EnvVarDependencyResolver<'a> {
    /// [EnvVarSet] with possibly dependent environment variables
    env_vars: &'a EnvVarSet,

    /// Maximum recursion depth
    ///
    /// Long dependency chains could slow down the operator.
    max_recursion_depth: usize,
}

impl<'a> EnvVarDependencyResolver<'a> {
    pub fn new(env_vars: &'a EnvVarSet, max_recursion_depth: usize) -> Self {
        Self {
            env_vars,
            max_recursion_depth,
        }
    }

    /// Returns a sort key for the given environment variable which considers dependencies to other
    /// environment variables
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::{
    /// #     collections::BTreeSet,
    /// #     str::FromStr,
    /// # };
    /// # use stackable_operator::{
    /// #     k8s_openapi::api::core::v1::{
    /// #         EnvVar, EnvVarSource, ObjectFieldSelector
    /// #     },
    /// #     v2::builder::pod::container::{
    /// #         EnvVarDependencyResolver, EnvVarSet
    /// #     },
    /// # };
    ///
    /// let env_var1 = EnvVar {
    ///     name: "ENV1".to_owned(),
    ///     value: Some("references to $(ENV2) and $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var2 = EnvVar {
    ///     name: "ENV2".to_owned(),
    ///     value: Some("reference to $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var3 = EnvVar {
    ///     name: "ENV3".to_owned(),
    ///     value: Some("reference to $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var4 = EnvVar {
    ///     name: "ENV4".to_owned(),
    ///     value: None,
    ///     value_from: Some(EnvVarSource {
    ///         field_ref: Some(ObjectFieldSelector {
    ///             field_path: "metadata.name".to_owned(),
    ///             ..ObjectFieldSelector::default()
    ///         }),
    ///         ..EnvVarSource::default()
    ///     }),
    /// };
    /// let env_var5 = EnvVar {
    ///     name: "ENV5".to_owned(),
    ///     value: Some("self reference to $(ENV5)".to_owned()),
    ///     value_from: None,
    /// };
    ///
    /// let env_vars = EnvVarSet::new()
    ///     .with_env_var(env_var1.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var2.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var3.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var4.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var5.clone())
    ///     .unwrap();
    ///
    /// let resolver = EnvVarDependencyResolver::new(&env_vars, 2);
    /// assert_eq!(
    ///     vec!["ENV4".to_owned(), "ENV2".to_owned(), "ENV1".to_owned()],
    ///     resolver.sort_key(&env_var1)
    /// );
    /// assert_eq!(
    ///     vec!["ENV4".to_owned(), "ENV2".to_owned()],
    ///     resolver.sort_key(&env_var2)
    /// );
    /// assert_eq!(
    ///     vec!["ENV4".to_owned(), "ENV3".to_owned()],
    ///     resolver.sort_key(&env_var3)
    /// );
    /// assert_eq!(vec!["ENV4".to_owned()], resolver.sort_key(&env_var4));
    /// assert_eq!(vec!["ENV5".to_owned()], resolver.sort_key(&env_var5));
    /// ```
    pub fn sort_key(&self, env_var: &EnvVar) -> Vec<String> {
        if let Some(mut closure) = self.calculate_closure(env_var) {
            // Add the name of the variable to its closure to make the set unique for every
            // variable.
            closure.insert(env_var.name.clone());

            closure.into_iter().rev().collect()
        } else {
            vec![env_var.name.clone()]
        }
    }

    /// Calculates the transitive closure of referenced environment variables
    ///
    /// If the given environment variable is part of a reference cycle or a reference chain longer
    /// than the maximum recursion depth, then `None` is returned.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::{
    /// #     collections::BTreeSet,
    /// #     str::FromStr,
    /// # };
    /// # use stackable_operator::{
    /// #     k8s_openapi::api::core::v1::{
    /// #         EnvVar, EnvVarSource, ObjectFieldSelector
    /// #     },
    /// #     v2::builder::pod::container::{
    /// #         EnvVarDependencyResolver, EnvVarSet
    /// #     },
    /// # };
    ///
    /// let env_var1 = EnvVar {
    ///     name: "ENV1".to_owned(),
    ///     value: Some("references to $(ENV2) and $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var2 = EnvVar {
    ///     name: "ENV2".to_owned(),
    ///     value: Some("reference to $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var3 = EnvVar {
    ///     name: "ENV3".to_owned(),
    ///     value: Some("reference to $(ENV4)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var4 = EnvVar {
    ///     name: "ENV4".to_owned(),
    ///     value: None,
    ///     value_from: Some(EnvVarSource {
    ///         field_ref: Some(ObjectFieldSelector {
    ///             field_path: "metadata.name".to_owned(),
    ///             ..ObjectFieldSelector::default()
    ///         }),
    ///         ..EnvVarSource::default()
    ///     }),
    /// };
    /// let env_var5 = EnvVar {
    ///     name: "ENV5".to_owned(),
    ///     value: Some("self reference to $(ENV5)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var6 = EnvVar {
    ///     name: "ENV6".to_owned(),
    ///     value: Some("cyclic reference to $(ENV7)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var7 = EnvVar {
    ///     name: "ENV7".to_owned(),
    ///     value: Some("cyclic reference to $(ENV6)".to_owned()),
    ///     value_from: None,
    /// };
    /// let env_var8 = EnvVar {
    ///     name: "ENV8".to_owned(),
    ///     value: Some("long reference chain to $(ENV1)".to_owned()),
    ///     value_from: None,
    /// };
    ///
    /// let env_vars = EnvVarSet::new()
    ///     .with_env_var(env_var1.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var2.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var3.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var4.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var5.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var6.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var7.clone())
    ///     .unwrap()
    ///     .with_env_var(env_var8.clone())
    ///     .unwrap();
    ///
    /// let resolver = EnvVarDependencyResolver::new(&env_vars, 2);
    /// assert_eq!(
    ///     Some(BTreeSet::from(["ENV2".to_owned(), "ENV4".to_owned()])),
    ///     resolver.calculate_closure(&env_var1)
    /// );
    /// assert_eq!(
    ///     Some(BTreeSet::from(["ENV4".to_owned()])),
    ///     resolver.calculate_closure(&env_var2)
    /// );
    /// assert_eq!(
    ///     Some(BTreeSet::from(["ENV4".to_owned()])),
    ///     resolver.calculate_closure(&env_var3)
    /// );
    /// assert_eq!(Some(BTreeSet::new()), resolver.calculate_closure(&env_var4));
    /// assert_eq!(None, resolver.calculate_closure(&env_var5));
    /// assert_eq!(None, resolver.calculate_closure(&env_var6));
    /// assert_eq!(None, resolver.calculate_closure(&env_var7));
    /// assert_eq!(None, resolver.calculate_closure(&env_var8));
    /// ```
    pub fn calculate_closure(&self, env_var: &EnvVar) -> Option<BTreeSet<String>> {
        self.calculate_closure_rec(env_var, self.max_recursion_depth)
    }

    fn calculate_closure_rec(
        &self,
        env_var: &EnvVar,
        remaining_recursion_depth: usize,
    ) -> Option<BTreeSet<String>> {
        if env_var.value.is_none() {
            Some(BTreeSet::new())
        } else if let Some(value) = &env_var.value
            && remaining_recursion_depth > 0
        {
            let mut closure = BTreeSet::new();

            for referenced_env_var in self.referenced_env_vars(value) {
                closure.insert(referenced_env_var.name.clone());
                closure.extend(
                    self.calculate_closure_rec(referenced_env_var, remaining_recursion_depth - 1)?,
                );
            }

            Some(closure)
        } else {
            None
        }
    }

    /// Returns the directly referenced environment variables
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::str::FromStr;
    /// # use stackable_operator::{
    /// #     k8s_openapi::api::core::v1::EnvVar,
    /// #     v2::builder::pod::container::{
    /// #         EnvVarDependencyResolver, EnvVarName, EnvVarSet
    /// #     },
    /// # };
    ///
    /// let env_vars = EnvVarSet::new().with_values([
    ///     (EnvVarName::from_str("ENV1").unwrap(), "value 1"),
    ///     (EnvVarName::from_str("ENV2").unwrap(), "value 2"),
    ///     (EnvVarName::from_str("ENV3").unwrap(), "value 3"),
    ///     (EnvVarName::from_str("ENV4").unwrap(), "value 4"),
    /// ]);
    ///
    /// let resolver = EnvVarDependencyResolver::new(&env_vars, 10);
    ///
    /// assert_eq!(
    ///     Vec::<&EnvVar>::new(),
    ///     resolver.referenced_env_vars("no references")
    /// );
    /// assert_eq!(
    ///     vec![
    ///         &EnvVar {
    ///             name: "ENV2".to_owned(),
    ///             value: Some("value 2".to_owned()),
    ///             value_from: None
    ///         },
    ///         &EnvVar {
    ///             name: "ENV3".to_owned(),
    ///             value: Some("value 3".to_owned()),
    ///             value_from: None
    ///         },
    ///     ],
    ///     resolver.referenced_env_vars("references to $(ENV2) and $(ENV3)")
    /// );
    /// assert_eq!(
    ///     vec![
    ///         &EnvVar {
    ///             name: "ENV1".to_owned(),
    ///             value: Some("value 1".to_owned()),
    ///             value_from: None
    ///         },
    ///         &EnvVar {
    ///             name: "ENV2".to_owned(),
    ///             value: Some("value 2".to_owned()),
    ///             value_from: None
    ///         },
    ///     ],
    ///     resolver.referenced_env_vars(
    ///         "references to $(ENV1) and $$$(ENV2) and escaped references to $$(ENV3) and $$$$(ENV4)"
    ///     )
    /// );
    /// assert_eq!(
    ///     vec![&EnvVar {
    ///         name: "ENV1".to_owned(),
    ///         value: Some("value 1".to_owned()),
    ///         value_from: None
    ///     }],
    ///     resolver.referenced_env_vars("reference to $(ENV1) and invalid reference to $(ENV5)")
    /// );
    /// ```
    pub fn referenced_env_vars(&self, value: &str) -> Vec<&'a EnvVar> {
        let value_without_escapes = ESCAPED_DOLLAR_SIGN_PATTERN.replace_all(value, "");

        REFERENCED_ENV_VARS_PATTERN
            .captures_iter(&value_without_escapes)
            .filter_map(|capture| capture.get(1))
            .filter_map(|regex_match| EnvVarName::from_str(regex_match.as_str()).ok())
            .filter_map(|env_var_name| self.env_vars.0.get(&env_var_name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{EnvVarName, EnvVarSet};
    use crate::{
        builder::pod::container::FieldPathEnvVar,
        k8s_openapi::api::core::v1::{
            ConfigMapKeySelector, EnvVar, EnvVarSource, ObjectFieldSelector,
        },
        v2::{
            builder::pod::container::new_container_builder,
            types::kubernetes::{ConfigMapKey, ConfigMapName, ContainerName},
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
    fn test_vec_envvar_from_envvarset() {
        let env_var_set = EnvVarSet::new()
            .with_value(&EnvVarName::from_str_unsafe("ENV1"), "$(ENV2)")
            .with_value(&EnvVarName::from_str_unsafe("ENV2"), "value 2")
            .with_value(&EnvVarName::from_str_unsafe("ENV3"), "value 3");

        assert_eq!(
            vec![
                EnvVar {
                    name: "ENV2".to_owned(),
                    value: Some("value 2".to_owned()),
                    value_from: None
                },
                EnvVar {
                    name: "ENV1".to_owned(),
                    value: Some("$(ENV2)".to_owned()),
                    value_from: None
                },
                EnvVar {
                    name: "ENV3".to_owned(),
                    value: Some("value 3".to_owned()),
                    value_from: None
                },
            ],
            Vec::from(env_var_set)
        );
    }

    #[test]
    fn test_envvarset_intoiterator() {
        let env_var_set = EnvVarSet::new()
            .with_value(&EnvVarName::from_str_unsafe("ENV1"), "$(ENV2)")
            .with_value(&EnvVarName::from_str_unsafe("ENV2"), "value 2")
            .with_value(&EnvVarName::from_str_unsafe("ENV3"), "value 3");

        let mut iter = env_var_set.into_iter();

        assert_eq!(
            Some(EnvVar {
                name: "ENV2".to_owned(),
                value: Some("value 2".to_owned()),
                value_from: None
            }),
            iter.next()
        );
        assert_eq!(
            Some(EnvVar {
                name: "ENV1".to_owned(),
                value: Some("$(ENV2)".to_owned()),
                value_from: None
            }),
            iter.next()
        );
        assert_eq!(
            Some(EnvVar {
                name: "ENV3".to_owned(),
                value: Some("value 3".to_owned()),
                value_from: None
            }),
            iter.next()
        );
        assert_eq!(None, iter.next());
    }
}
