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
use crate::krustlet;

use std::collections::HashMap;

use crate::client::Client;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use tracing::debug;

pub struct RoleGroup {
    pub name: String,
    pub selector: LabelSelector,
}

pub async fn find_nodes_that_fit_selectors(
    client: &Client,
    namespace: Option<String>,
    role_groups: Vec<RoleGroup>,
) -> OperatorResult<HashMap<String, Vec<Node>>> {
    let mut found_nodes = HashMap::new();
    for role_group in role_groups {
        let selector = krustlet::add_stackable_selector(&role_group.selector);
        let nodes = client
            .list_with_label_selector(namespace.clone().as_deref(), &selector)
            .await?;
        debug!(
            "Found [{}] nodes for role group [{}]: [{:?}]",
            nodes.len(),
            role_group.name,
            nodes
        );
        found_nodes.insert(role_group.name.clone(), nodes);
    }
    Ok(found_nodes)
}
