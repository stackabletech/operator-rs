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
use crate::error::{Error, OperatorResult};
use crate::product_config_utils::Configuration;
use derivative::Derivative;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{runtime::reflector::ObjectRef, Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct Role<T: Sized> {
    #[serde(flatten)]
    pub config: CommonConfiguration<T>,
    pub role_groups: HashMap<String, RoleGroup<T>>,
}

impl<T: Clone + Merge> Role<T> {
    pub fn convert_and_merge<C: Configuration + From<T>>(
        role_name: &str,
        optional_role: &Role<T>,
    ) -> OperatorResult<Role<C>> {
        let mut merged_groups: HashMap<String, RoleGroup<C>> = HashMap::new();
        for (role_group_name, role_group) in &optional_role.role_groups {
            merged_groups.insert(
                role_group_name.clone(),
                RoleGroup {
                    replicas: role_group.replicas,
                    selector: role_group.selector.clone(),
                    config: optional_role.merge_common_config(role_name, role_group_name)?,
                },
            );
        }

        Ok(Role {
            config: CommonConfiguration {
                config: optional_role.config.config.clone().into(),
                config_overrides: optional_role.config.config_overrides.clone(),
                env_overrides: optional_role.config.env_overrides.clone(),
                cli_overrides: optional_role.config.cli_overrides.clone(),
            },
            role_groups: merged_groups,
        })
    }

    fn merge_common_config<C: Configuration + From<T>>(
        &self,
        role: &str,
        role_group: &str,
    ) -> OperatorResult<CommonConfiguration<C>> {
        let role_config = &self.config;
        let group_config = &self
            .role_groups
            .get(role_group)
            .ok_or(Error::MissingRoleGroup {
                role: role.to_string(),
                role_group: role_group.to_string(),
            })?
            .config;

        Ok(CommonConfiguration {
            config: Self::merge_config(&role_config.config, &group_config.config).into(),
            config_overrides: Self::merge_config_file_overrides(role_config, group_config),
            env_overrides: Self::merge_env_overrides(role_config, group_config),
            cli_overrides: Self::merge_cli_overrides(role_config, group_config),
        })
    }

    fn merge_config_file_overrides(
        role_config: &CommonConfiguration<T>,
        role_group_config: &CommonConfiguration<T>,
    ) -> HashMap<String, HashMap<String, String>> {
        let mut merge_result: HashMap<String, HashMap<String, String>> = HashMap::new();

        if !role_config.config_overrides.is_empty() {
            for (file_name, role_config_overrides) in &role_config.config_overrides {
                if let Some(role_group_config_overrides) =
                    role_group_config.config_overrides.get(file_name)
                {
                    // file exists in role config and role group config
                    let mut merged = role_config_overrides.clone();
                    merged.extend(role_group_config_overrides.clone());
                    merge_result.insert(file_name.clone(), merged);
                } else {
                    // only role has the specified file
                    merge_result.insert(file_name.clone(), role_config_overrides.clone());
                }
            }
        } else {
            merge_result = role_group_config.config_overrides.clone();
        }

        merge_result
    }

    fn merge_env_overrides(
        role_config: &CommonConfiguration<T>,
        role_group_config: &CommonConfiguration<T>,
    ) -> HashMap<String, String> {
        let mut merge_result = role_config.env_overrides.clone();
        merge_result.extend(role_group_config.env_overrides.clone());
        merge_result
    }

    fn merge_cli_overrides(
        role_config: &CommonConfiguration<T>,
        role_group_config: &CommonConfiguration<T>,
    ) -> BTreeMap<String, String> {
        let mut merge_result = role_config.cli_overrides.clone();
        merge_result.extend(role_group_config.cli_overrides.clone());
        merge_result
    }

    fn merge_config(role_config: &T, role_group_config: &T) -> T {
        let mut merge_result = role_group_config.clone();
        merge_result.merge(role_config);
        merge_result
    }
}

impl<T: Configuration + 'static> Role<T> {
    pub fn role_group_config(
        &self,
        role: &str,
        role_group: &str,
    ) -> OperatorResult<&CommonConfiguration<T>> {
        Ok(&self
            .role_groups
            .get(role_group)
            .ok_or(Error::MissingRoleGroup {
                role: role.to_string(),
                role_group: role_group.to_string(),
            })?
            .config)
    }

    /// This casts a generic struct implementing [`crate::product_config_utils::Configuration`]
    /// and used in [`Role`] into a Box of a dynamically dispatched
    /// [`crate::product_config_utils::Configuration`] Trait. This is required to use the generic
    /// [`Role`] with more than a single generic struct. For example different roles most likely
    /// have different structs implementing Configuration.
    pub fn erase(self) -> Role<Box<dyn Configuration<Configurable = T::Configurable>>> {
        Role {
            config: CommonConfiguration {
                config: Box::new(self.config.config)
                    as Box<dyn Configuration<Configurable = T::Configurable>>,
                config_overrides: self.config.config_overrides,
                env_overrides: self.config.env_overrides,
                cli_overrides: self.config.cli_overrides,
            },
            role_groups: self
                .role_groups
                .into_iter()
                .map(|(name, group)| {
                    (
                        name,
                        RoleGroup {
                            config: CommonConfiguration {
                                config: Box::new(group.config.config)
                                    as Box<dyn Configuration<Configurable = T::Configurable>>,
                                config_overrides: group.config.config_overrides,
                                env_overrides: group.config.env_overrides,
                                cli_overrides: group.config.cli_overrides,
                            },
                            replicas: group.replicas,
                            selector: group.selector,
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct RoleGroup<T> {
    #[serde(flatten)]
    pub config: CommonConfiguration<T>,
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
