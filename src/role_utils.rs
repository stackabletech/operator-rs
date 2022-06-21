//! This module provides utility functions for dealing with role (types) and role groups.
//!
//! While other modules in this crate try to be generic and reusable for other operators
//! this one makes very specific assumptions about how a CRD is structured.
//!
//! These assumptions are detailed and explained below.
//!
//! # Roles / Role types
//!
//! A CRD is often used to operate another piece of software.
//! Software - especially the distributed kind - sometimes consists of multiple different types of program working together to achieve their goal.
//! These different types are what we call a _role_.
//!
//! ## Examples
//!
//! Apache Hadoop HDFS:
//! * NameNode
//! * DataNode
//! * JournalNode
//!
//! Kubernetes:
//! * kube-apiserver
//! * kubelet
//! * kube-controller-manager
//! * ...
//!
//! # Role Groups
//!
//! There is sometimes a need to have different configuration options or different label selectors for different replicas of the same role.
//! Role groups are what allows this.
//! Nested under a role there can be multiple role groups, each with its own LabelSelector and configuration.
//!
//! ## Example
//!
//! This example has one role (`leader`) and two role groups (`default`, and `20core`)
//!
//! ```yaml
//!   leader:
//!     roleGroups:
//!       default:
//!         selector:
//!           matchLabels:
//!             component: spark
//!           matchExpressions:
//!             - { key: tier, operator: In, values: [ cache ] }
//!             - { key: environment, operator: NotIn, values: [ dev ] }
//!         config:
//!           cores: 1
//!           memory: "1g"
//!         replicas: 3
//!       20core:
//!         selector:
//!           matchLabels:
//!             component: spark
//!             cores: 20
//!           matchExpressions:
//!             - { key: tier, operator: In, values: [ cache ] }
//!             - { key: environment, operator: NotIn, values: [ dev ] }
//!           config:
//!             cores: 10
//!             memory: "1g"
//!           replicas: 3
//!     config:
//! ```
//!
//! # Pod labels
//!
//! Each Pod that Operators create needs to have a common set of labels.
//! These labels are (with one exception) listed in the Kubernetes [documentation](https://kubernetes.io/docs/concepts/overview/working-with-objects/common-labels/):
//!
//! * app.kubernetes.io/name - The name of the application. This will usually be a static string (e.g. "zookeeper").
//! * app.kubernetes.io/instance - The name of the parent resource, this is useful so an operator can list all its pods by using a LabelSelector
//! * app.kubernetes.io/version - The current version of the application
//! * app.kubernetes.io/component - The role/role type, this is used to distinguish multiple pods on the same node from each other
//! * app.kubernetes.io/part-of - The name of a higher level application this one is part of. We have decided to leave this empty for now.
//! * app.kubernetes.io/managed-by - The tool being used to manage the operation of an application (e.g. "zookeeper-operator")
//! * app.kubernetes.io/role-group - The name of the role group this pod belongs to
//!
//! NOTE: We find the official description to be ambiguous so we use these labels as defined above.
//!
//! Each resource can have more operator specific labels.

use crate::config::merge::Merge;
use crate::product_config_utils::Configuration;
use derivative::Derivative;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{runtime::reflector::ObjectRef, Resource};
use schemars::JsonSchema;
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Debug, Display},
};

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct CommonConfiguration<T: Sized> {
    #[serde(default)]
    // We can't depend on T being `Default`, since that trait is not object-safe
    // We only need to generate schemas for fully specified types, but schemars_derive
    // does not support specifying custom bounds.
    #[schemars(default = "config_schema_default")]
    pub config: T,
    #[serde(default)]
    pub config_overrides: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub env_overrides: HashMap<String, String>,
    // BTreeMap to keep some order with the cli arguments.
    #[serde(default)]
    pub cli_overrides: BTreeMap<String, String>,
}

fn config_schema_default() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Config<O: Clone + Default + Merge, S: Configuration + From<O>> {
    Optional(CommonConfiguration<O>),
    #[serde(skip)]
    Standard(CommonConfiguration<S>),
}

impl<O: Clone + Default + Merge, S: Configuration + From<O>> Default for Config<O, S> {
    fn default() -> Self {
        Config::Optional(CommonConfiguration::default())
    }
}

impl<O: Clone + Default + Merge, S: Clone + Configuration + From<O>> Config<O, S> {
    pub fn to_standard(&self) -> Self {
        match &self {
            Config::Optional(opt) => Config::Standard(CommonConfiguration::<S> {
                config: opt.config.clone().into(),
                config_overrides: opt.config_overrides.clone(),
                env_overrides: opt.env_overrides.clone(),
                cli_overrides: opt.cli_overrides.clone(),
            }),
            Config::Standard(std) => Config::Standard(CommonConfiguration::<S> {
                config: std.config.clone(),
                config_overrides: std.config_overrides.clone(),
                env_overrides: std.env_overrides.clone(),
                cli_overrides: std.cli_overrides.clone(),
            }),
        }
    }
}

impl<O: Clone + Merge> Merge for CommonConfiguration<O> {
    fn merge(&mut self, defaults: &Self) {
        // merge configs
        self.config.merge(&defaults.config);
        // merge overrides
        // file
        let mut merged_config_overrides: HashMap<String, HashMap<String, String>> = HashMap::new();

        if !defaults.config_overrides.is_empty() {
            for (file_name, default_overrides) in &defaults.config_overrides {
                if let Some(self_config_overrides) = self.config_overrides.get(file_name) {
                    // file exists in role config and role group config
                    let mut merged = default_overrides.clone();
                    merged.extend(self_config_overrides.clone());
                    merged_config_overrides.insert(file_name.clone(), merged);
                } else {
                    // only role has the specified file
                    merged_config_overrides.insert(file_name.clone(), default_overrides.clone());
                }
            }
        } else {
            merged_config_overrides = self.config_overrides.clone();
        }

        self.config_overrides = merged_config_overrides;
        // env
        let mut default_env_overrides = defaults.env_overrides.clone();
        default_env_overrides.extend(self.env_overrides.clone());
        self.env_overrides = default_env_overrides;
        // cli
        let mut default_cli_overrides = defaults.cli_overrides.clone();
        default_cli_overrides.extend(self.cli_overrides.clone());
        self.cli_overrides = default_cli_overrides;
    }
}

impl<O: Clone + Default + Merge, S: Configuration + From<O>> Merge for Config<O, S> {
    fn merge(&mut self, defaults: &Self) {
        match (self, defaults) {
            (Self::Optional(self_opt), Self::Optional(default_opt)) => {
                self_opt.merge(default_opt);
            }
            (_, _) => {
                // TODO: panic?
                panic!("Can not merge non optional config structs!")
            }
        }
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct Role<O: Clone + Default + Merge + Sized, S: Clone + Configuration + From<O>> {
    #[serde(flatten)]
    pub config: Config<O, S>,
    pub role_groups: HashMap<String, RoleGroup<O, S>>,
}

impl<'de, O, S> Deserialize<'de> for Role<O, S>
where
    O: Clone + Debug + Default + Deserialize<'de> + Merge,
    S: Clone + Debug + Configuration + Deserialize<'de> + From<O>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const CONFIG_FIELD: &str = "config";
        const ROLE_GROUP_FIELD: &str = "roleGroups";

        struct RoleVisitor<O, S> {
            o: PhantomData<O>,
            s: PhantomData<S>,
        }

        impl<'de, O, S> Visitor<'de> for RoleVisitor<O, S>
        where
            O: Clone + Debug + Default + Deserialize<'de> + Merge,
            S: Clone + Configuration + Debug + Deserialize<'de> + From<O>,
        {
            type Value = Role<O, S>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                todo!()
            }

            fn visit_map<M>(self, mut access: M) -> Result<Role<O, S>, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut config: Option<Config<O, S>> = None;
                let mut role_groups: Option<HashMap<String, RoleGroup<O, S>>> = None;

                while let Some(key) = access.next_key::<String>()? {
                    match key.as_ref() {
                        CONFIG_FIELD => {
                            if config.is_some() {
                                return Err(<M::Error as Error>::duplicate_field(CONFIG_FIELD));
                            }
                            config = Some(access.next_value()?);
                        }
                        ROLE_GROUP_FIELD => {
                            if role_groups.is_some() {
                                return Err(<M::Error as Error>::duplicate_field(ROLE_GROUP_FIELD));
                            }
                            role_groups = Some(access.next_value()?);
                        }
                        name => {
                            return Err(<M::Error as Error>::unknown_field(
                                name,
                                &[CONFIG_FIELD, ROLE_GROUP_FIELD],
                            ));
                        }
                    }
                }

                let config = match config {
                    Some(config) => config,
                    None => return Err(<M::Error as Error>::missing_field(CONFIG_FIELD)),
                };
                let role_groups = match role_groups {
                    Some(role_groups) => role_groups,
                    None => return Err(<M::Error as Error>::missing_field(ROLE_GROUP_FIELD)),
                };

                // merging....
                let mut merged_groups: HashMap<String, RoleGroup<O, S>> = HashMap::new();

                for (role_group_name, mut role_group) in &role_groups {
                    let mut merged_config = role_group.config.clone();
                    println!("role: {:#?}", config);
                    println!("role_group: {:#?}", merged_config);
                    merged_config.merge(&config);
                    println!("role_group_merged: {:#?}", merged_config);
                    merged_groups.insert(
                        role_group_name.clone(),
                        RoleGroup {
                            replicas: role_group.replicas,
                            selector: role_group.selector.clone(),
                            config: merged_config.to_standard(),
                        },
                    );
                }

                Ok(Role {
                    config: config.to_standard(),
                    role_groups: merged_groups,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &[CONFIG_FIELD, ROLE_GROUP_FIELD];
        deserializer.deserialize_struct(
            "Role",
            FIELDS,
            RoleVisitor {
                o: PhantomData::default(),
                s: PhantomData::default(),
            },
        )
    }
}

/*
impl<O: Clone + Merge, S: Clone + Configuration + From<O> + 'static> Role<O, S>
where
    Box<(dyn Configuration<Configurable = <S as Configuration>::Configurable> + 'static)>: From<O>,
{
    /// This casts a generic struct implementing [`crate::product_config_utils::Configuration`]
    /// and used in [`Role`] into a Box of a dynamically dispatched
    /// [`crate::product_config_utils::Configuration`] Trait. This is required to use the generic
    /// [`Role`] with more than a single generic struct. For example different roles most likely
    /// have different structs implementing Configuration.
    pub fn erase(self) -> Role<O, Box<dyn Configuration<Configurable = S::Configurable>>> {
        Role {
            config: Config::Standard(CommonConfiguration {
                config: Box::new(self.config.config)
                    as Box<dyn Configuration<Configurable = S::Configurable>>,
                config_overrides: self.config.config_overrides,
                env_overrides: self.config.env_overrides,
                cli_overrides: self.config.cli_overrides,
            }),
            role_groups: self
                .role_groups
                .into_iter()
                .map(|(name, group)| {
                    (
                        name,
                        RoleGroup {
                            config: Config::Standard(CommonConfiguration {
                                config: Box::new(group.config.config)
                                    as Box<dyn Configuration<Configurable = S::Configurable>>,
                                config_overrides: group.config.config_overrides,
                                env_overrides: group.config.env_overrides,
                                cli_overrides: group.config.cli_overrides,
                            }),
                            replicas: group.replicas,
                            selector: group.selector,
                        },
                    )
                })
                .collect(),
        }
    }
}
*/

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleGroup<O: Clone + Default + Merge + Sized, S: Sized + Configuration + From<O>> {
    #[serde(flatten)]
    pub config: Config<O, S>,
    pub replicas: Option<u16>,
    pub selector: Option<LabelSelector>,
}

/// A reference to a named role group of a given cluster object
#[derive(Derivative)]
#[derivative(
    Debug(bound = "K::DynamicType: Debug"),
    Clone(bound = "K::DynamicType: Clone")
)]
pub struct RoleGroupRef<K: Resource> {
    pub cluster: ObjectRef<K>,
    pub role: String,
    pub role_group: String,
}

impl<K: Resource> RoleGroupRef<K> {
    pub fn object_name(&self) -> String {
        format!("{}-{}-{}", self.cluster.name, self.role, self.role_group)
    }
}

impl<K: Resource> Display for RoleGroupRef<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "role group {}/{} of {}",
            self.role, self.role_group, self.cluster
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product_config_utils::ConfigResult;

    #[derive(Clone, Deserialize, Default, Debug, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Test {
        port: u16,
    }

    impl Configuration for Test {
        type Configurable = ();

        fn compute_env(
            &self,
            resource: &Self::Configurable,
            role_name: &str,
        ) -> ConfigResult<BTreeMap<String, Option<String>>> {
            todo!()
        }

        fn compute_cli(
            &self,
            resource: &Self::Configurable,
            role_name: &str,
        ) -> ConfigResult<BTreeMap<String, Option<String>>> {
            todo!()
        }

        fn compute_files(
            &self,
            resource: &Self::Configurable,
            role_name: &str,
            file: &str,
        ) -> ConfigResult<BTreeMap<String, Option<String>>> {
            todo!()
        }
    }

    #[derive(Clone, Deserialize, Default, Debug, Merge, JsonSchema, PartialEq, Serialize)]
    #[merge(path_overrides(merge = "crate::config::merge"))]
    #[serde(rename_all = "camelCase")]
    pub struct OptTest {
        port: Option<u16>,
    }

    const DEFAULT_PORT: u16 = 33333;
    impl From<OptTest> for Test {
        fn from(opt: OptTest) -> Self {
            Self {
                port: opt.port.unwrap_or(DEFAULT_PORT),
            }
        }
    }

    #[test]
    fn test() {
        let role: Role<OptTest, Test> = serde_yaml::from_str(
            r#"
config:
  config:
    port: 11111
roleGroups:
  default:
    config: {}
    "#,
        )
        .unwrap();

        eprintln!("{:?}", role);
    }
}
