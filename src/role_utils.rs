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
use crate::{krustlet, labels};

use std::collections::{BTreeMap, HashMap};

use crate::client::Client;
use crate::k8s_utils::LabelOptionalValueMap;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use tracing::{debug, trace};

pub struct RoleGroup {
    pub name: String,
    pub selector: LabelSelector,
}

/// Return a map where the key corresponds to the role_group (e.g. "default", "10core10Gb") and
/// a vector of nodes that fit the role_groups selector description.
pub async fn find_nodes_that_fit_selectors(
    client: &Client,
    namespace: Option<String>,
    role_groups: &[RoleGroup],
) -> OperatorResult<HashMap<String, Vec<Node>>> {
    let mut found_nodes = HashMap::new();
    for role_group in role_groups {
        let selector = krustlet::add_stackable_selector(&role_group.selector);
        let nodes = client
            .list_with_label_selector(namespace.as_deref(), &selector)
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

/// For each role, return a tuple consisting of eligible nodes for a given selector.
/// Required to delete excess pods that do not match any node or selector description.
pub fn get_full_pod_node_map(
    eligible_nodes: &HashMap<String, HashMap<String, Vec<Node>>>,
) -> Vec<(Vec<Node>, LabelOptionalValueMap)> {
    let mut eligible_nodes_for_role_and_group = vec![];
    for (role, eligible_nodes_for_role) in eligible_nodes {
        for (group_name, eligible_nodes) in eligible_nodes_for_role {
            trace!(
                "Adding {} nodes to eligible node list for role [{}] and group [{}].",
                eligible_nodes.len(),
                role,
                group_name
            );
            eligible_nodes_for_role_and_group.push((
                eligible_nodes.clone(),
                get_role_and_group_labels(role, group_name),
            ))
        }
    }

    eligible_nodes_for_role_and_group
}

/// Return a map with labels and values for role (component) and group (role_group).
/// Required to find nodes that are a possible match for pods.
fn get_role_and_group_labels(role: &str, group_name: &str) -> LabelOptionalValueMap {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_full_pod_node_map() {
        let role = "server";
        let group_name = "default";
        let group_name_2 = "default2";

        let mut node_1 = Node::default();
        node_1.metadata.name = Some("node_1".to_string());

        let mut node_2 = Node::default();
        node_2.metadata.name = Some("node_2".to_string());

        let mut node_3 = Node::default();
        node_3.metadata.name = Some("node_3".to_string());

        let node_vec_1_2 = vec![node_1, node_2];
        let node_vec_3 = vec![node_3];

        let mut node_group = HashMap::new();
        node_group.insert(group_name.to_string(), node_vec_1_2.clone());
        node_group.insert(group_name_2.to_string(), node_vec_3.clone());

        let mut eligible_nodes: HashMap<String, HashMap<String, Vec<Node>>> = HashMap::new();
        eligible_nodes.insert(role.to_string(), node_group);

        let full_pod_node_map = get_full_pod_node_map(&eligible_nodes);

        for (nodes, labels) in full_pod_node_map {
            if let Some(role_group_label) = labels.get(labels::APP_ROLE_GROUP_LABEL).unwrap() {
                if role_group_label == group_name {
                    assert_eq!(nodes.len(), node_vec_1_2.len())
                }
                if role_group_label == group_name_2 {
                    assert_eq!(nodes.len(), node_vec_3.len())
                }
            }

            if let Some(role_label) = labels.get(labels::APP_COMPONENT_LABEL).unwrap() {
                assert_eq!(role_label, role);
            }
        }
    }

    #[test]
    fn test_get_role_and_group_labels() {
        let role = "server";
        let group_name = "default";

        let result = get_role_and_group_labels(role, group_name);

        assert_eq!(result.len(), 2);

        let mut expected = BTreeMap::new();
        expected.insert(
            labels::APP_COMPONENT_LABEL.to_string(),
            Some(role.to_string()),
        );
        expected.insert(
            labels::APP_ROLE_GROUP_LABEL.to_string(),
            Some(group_name.to_string()),
        );

        assert_eq!(result, expected);
    }
}
