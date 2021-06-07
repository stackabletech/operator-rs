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
//! There is sometimes a need to have different configuration options or different label selectors for different instances of the same role.
//! Role groups are what allows this.
//! Nested under a role there can be multiple role groups, each with its own LabelSelector and configuration.
//!
//! ## Example
//!
//! This example has one role (`leader`) and two role groups (`default`, and `20core`)
//!
//! ```yaml
//!  leader:
//     selectors:
//       default:
//         selector:
//           matchLabels:
//             component: spark
//           matchExpressions:
//             - { key: tier, operator: In, values: [ cache ] }
//             - { key: environment, operator: NotIn, values: [ dev ] }
//         config:
//           cores: 1
//           memory: "1g"
//         instances: 3
//         instancesPerNode: 1
//       20core:
//         selector:
//           matchLabels:
//             component: spark
//             cores: 20
//           matchExpressions:
//             - { key: tier, operator: In, values: [ cache ] }
//             - { key: environment, operator: NotIn, values: [ dev ] }
//           config:
//             cores: 10
//             memory: "1g"
//           instances: 3
//           instancesPerNode: 2
//     config:
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
//! * app.kubernetes.io/part-of - The name of a higher level application this one is part of. In our case this will usually be the same as `name`
//! * app.kubernetes.io/managed-by - The tool being used to manage the operation of an application (e.g. "zookeeper-operator")
//! * app.kubernetes.io/role-group - The name of the role group this pod belongs to
//!
//! NOTE: We find the official description to be ambiguous so we use these labels as defined above.
//!
//! Each resource can have more operator specific labels.

use crate::error::OperatorResult;
use crate::{krustlet, label_selector};

use std::collections::HashMap;

use crate::client::Client;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::debug;

// TODO: This is an unused idea on how to support ignoring errors on validation
pub enum Property {
    Simple(String),
    Complex {
        ignore_warning: bool,
        ignore_error: bool,
        value: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonConfiguration<T> {
    pub config: Option<T>,
    pub config_overrides: Option<HashMap<String, HashMap<String, String>>>,
    pub env_overrides: Option<HashMap<String, String>>,
    pub cli_overrides: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Role<T> {
    #[serde(flatten)]
    pub config: Option<CommonConfiguration<T>>,
    pub role_groups: HashMap<String, RoleGroup<T>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleGroup<T> {
    #[serde(flatten)]
    pub config: Option<CommonConfiguration<T>>,
    // TODO: In Kubernetes this is called `replicas` should we stay closer to that?
    pub instances: u16,
    #[schemars(schema_with = "label_selector::schema")]
    pub selector: Option<LabelSelector>,
}

pub async fn find_nodes_that_fit_selectors<T>(
    client: &Client,
    namespace: Option<String>,
    role: &Role<T>,
) -> OperatorResult<HashMap<String, Vec<Node>>>
where
    T: Serialize,
{
    let mut found_nodes = HashMap::new();
    for (group_name, role_group) in &role.role_groups {
        let selector = krustlet::add_stackable_selector(role_group.selector.as_ref());
        let nodes = client
            .list_with_label_selector(namespace.as_deref(), &selector)
            .await?;
        debug!(
            "Found [pa{}] nodes for role group [{}]: [{:?}]",
            nodes.len(),
            group_name,
            nodes
        );
        found_nodes.insert(group_name.clone(), nodes);
    }
    Ok(found_nodes)
}
