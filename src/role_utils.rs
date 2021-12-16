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
#![allow(deprecated)]
use crate::error::OperatorResult;
use crate::labels;

use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Debug, Display},
};

use crate::client::Client;
#[allow(deprecated)]
use crate::k8s_utils::LabelOptionalValueMap;
use crate::product_config_utils::Configuration;
use derivative::Derivative;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::{runtime::reflector::ObjectRef, Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

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

impl<T: Configuration + 'static> Role<T> {
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

/// Return a map where the key corresponds to the role_group (e.g. "default", "10core10Gb") and
/// a tuple of a vector of nodes that fit the role_groups selector description, and the role_groups
/// "replicas" field for scheduling missing pods or removing excess pods.
#[deprecated(
    since = "0.5.0",
    note = "Should not be needed anymore after move to statefulsets"
)]
#[allow(deprecated)]
pub async fn find_nodes_that_fit_selectors<T>(
    client: &Client,
    namespace: Option<String>,
    role: &Role<T>,
) -> OperatorResult<HashMap<String, EligibleNodesAndReplicas>>
where
    T: Serialize,
{
    let mut found_nodes = HashMap::new();
    for (group_name, role_group) in &role.role_groups {
        let selector = role_group.selector.to_owned().unwrap_or_default();
        let nodes = client
            .list_with_label_selector(namespace.as_deref(), &selector)
            .await?;
        debug!(
            "Found [{}] nodes for role group [{}]: [{:?}]",
            nodes.len(),
            group_name,
            nodes
        );
        found_nodes.insert(
            group_name.clone(),
            EligibleNodesAndReplicas {
                nodes,
                replicas: role_group.replicas,
            },
        );
    }
    Ok(found_nodes)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[deprecated(
    since = "0.5.0",
    note = "Should not be needed anymore after move to statefulsets"
)]
pub struct EligibleNodesAndReplicas {
    pub nodes: Vec<Node>,
    pub replicas: Option<u16>,
}

/// Type to avoid clippy warnings
/// HashMap<`NameOfRole`, HashMap<`NameOfRoleGroup`, EligibleNodesAndReplicas(Vec<`Node`>, Option<`Replicas`>)>>
#[deprecated(
    since = "0.5.0",
    note = "Should not be needed anymore after move to statefulsets"
)]
#[allow(deprecated)]
pub type EligibleNodesForRoleAndGroup = HashMap<String, HashMap<String, EligibleNodesAndReplicas>>;

/// Return a list of eligible nodes and the provided replica count for each role and group
/// combination. Required to delete excess pods that do not match any node, selector description
/// or exceed the replica count.
///
/// # Arguments
/// * `eligible_nodes` - Represents the mappings for role on role_groups on nodes and replicas:
///                      HashMap<`NameOfRole`, HashMap<`NameOfRoleGroup`, (Vec<`Node`>, Option<`Replicas`>)>>
#[deprecated(
    since = "0.5.0",
    note = "Should not be needed anymore after move to statefulsets"
)]
#[allow(deprecated)]
pub fn list_eligible_nodes_for_role_and_group(
    eligible_nodes: &EligibleNodesForRoleAndGroup,
) -> Vec<(Vec<Node>, LabelOptionalValueMap, Option<u16>)> {
    let mut eligible_nodes_for_role_and_group = vec![];
    for (role, eligible_nodes_for_role) in eligible_nodes {
        for (group_name, eligible_nodes) in eligible_nodes_for_role {
            trace!(
                "Adding {} nodes to eligible node list for role [{}] and group [{}].",
                eligible_nodes.nodes.len(),
                role,
                group_name
            );
            eligible_nodes_for_role_and_group.push((
                eligible_nodes.nodes.clone(),
                get_role_and_group_labels(role, group_name),
                eligible_nodes.replicas,
            ))
        }
    }

    eligible_nodes_for_role_and_group
}

/// Return a map with labels and values for role (component) and group (role_group).
#[deprecated(
    since = "0.5.0",
    note = "Should not be needed anymore after move to statefulsets"
)]
#[allow(deprecated)]
pub fn get_role_and_group_labels(role: &str, group_name: &str) -> LabelOptionalValueMap {
    let mut labels = BTreeMap::new();
    labels.insert(
        labels::APP_COMPONENT_LABEL.to_string(),
        Some(role.to_string()),
    );
    labels.insert(
        labels::APP_ROLE_GROUP_LABEL.to_string(),
        Some(group_name.to_string()),
    );
    labels
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
