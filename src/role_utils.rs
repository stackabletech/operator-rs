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
//!     selectors:
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
//! * app.kubernetes.io/part-of - The name of a higher level application this one is part of. In our case this will usually be the same as `name`
//! * app.kubernetes.io/managed-by - The tool being used to manage the operation of an application (e.g. "zookeeper-operator")
//! * app.kubernetes.io/role-group - The name of the role group this pod belongs to
//!
//! NOTE: We find the official description to be ambiguous so we use these labels as defined above.
//!
//! Each resource can have more operator specific labels.

use crate::error::OperatorResult;
use crate::{krustlet, label_selector, labels};

use std::collections::{BTreeMap, HashMap};

use crate::client::Client;
use crate::k8s_utils::LabelOptionalValueMap;
use crate::product_config_utils::Configuration;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonConfiguration<T: Sized> {
    pub config: Option<T>,
    pub config_overrides: Option<HashMap<String, HashMap<String, String>>>,
    pub env_overrides: Option<HashMap<String, String>>,
    // BTreeMap to keep some order with the cli arguments.
    pub cli_overrides: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Role<T: Sized> {
    #[serde(flatten)]
    pub config: Option<CommonConfiguration<T>>,
    pub role_groups: HashMap<String, RoleGroup<T>>,
}

impl<T> Role<T>
where
    T: Configuration + 'static,
{
    pub fn into_dyn(self) -> Role<Box<dyn Configuration<Configurable = T::Configurable>>> {
        Role {
            config: self.config.map(|common| CommonConfiguration {
                config: common.config.map(|cfg| {
                    Box::new(cfg) as Box<dyn Configuration<Configurable = T::Configurable>>
                }),
                config_overrides: common.config_overrides,
                env_overrides: common.env_overrides,
                cli_overrides: common.cli_overrides,
            }),
            role_groups: self
                .role_groups
                .into_iter()
                .map(|(name, group)| {
                    (
                        name,
                        RoleGroup {
                            config: group.config.map(|common| CommonConfiguration {
                                config: common.config.map(|cfg| {
                                    Box::new(cfg)
                                        as Box<dyn Configuration<Configurable = T::Configurable>>
                                }),
                                config_overrides: common.config_overrides,
                                env_overrides: common.env_overrides,
                                cli_overrides: common.cli_overrides,
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleGroup<T> {
    #[serde(flatten)]
    pub config: Option<CommonConfiguration<T>>,
    pub replicas: u16,
    #[schemars(schema_with = "label_selector::schema")]
    pub selector: Option<LabelSelector>,
}

/// Return a map where the key corresponds to the role_group (e.g. "default", "10core10Gb") and
/// a vector of nodes that fit the role_groups selector description.
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
            "Found [{}] nodes for role group [{}]: [{:?}]",
            nodes.len(),
            group_name,
            nodes
        );
        found_nodes.insert(group_name.clone(), nodes);
    }
    Ok(found_nodes)
}

/// Return a list of eligible nodes for each role and group combination.
/// Required to delete excess pods that do not match any node or selector description.
pub fn list_eligible_nodes_for_role_and_group(
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case::one_role(
        r#"
        role_1:
          group_1:
            - node_1
            - node_2
          group2:
            - node_3"
    "#
    )]
    #[case::two_roles(
        r#"
        role_1:
          group_1:
            - node_1
            - node_2
        role_2:
          group_2:
            - node_1
          group_3:
            - node_3
    "#
    )]
    #[case::three_roles_many_nodes(
        r#"
        role_1:
          group_1:
            - node_1
            - node_2
            - node_3
          group_2:
            - node_1
            - node_4
        role_2:
          group_1:
            - node_1
            - node_4
          group_4:
            - node_2
            - node_3
        role_3:
          group_1:
            - node_2
            - node_4
          group_4: 
            - node_4
            - node_2
            - node_3
            - node_1
    "#
    )]
    #[trace]
    fn test_list_eligible_nodes_for_role_and_group(#[case] eligible_node_names: &str) {
        let eligible_node_names_parsed: HashMap<String, HashMap<String, Vec<String>>> =
            serde_yaml::from_str(eligible_node_names).expect("Invalid test definition!");

        // We need to map the innermost `String` objects to `Node` objects, but to get to them
        // a couple of nested loops are required
        // The entire purpose of this code is to transform `HashMap<String, HashMap<String, Vec<String>>>`
        // into `HashMap<String, HashMap<String, Vec<Node>>>`
        let eligible_nodes: HashMap<String, HashMap<String, Vec<Node>>> =
            eligible_node_names_parsed
                .iter()
                .map(|(role, role_groups)| {
                    (
                        role.clone(),
                        role_groups
                            .iter()
                            .map(|(group_name, nodes)| {
                                (
                                    group_name.clone(),
                                    nodes
                                        .iter()
                                        .map(|node_name| {
                                            let mut node = Node::default();
                                            node.metadata.name = Some(node_name.clone());
                                            node
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            })
                            .collect::<HashMap<_, _>>(),
                    )
                })
                .collect::<HashMap<_, _>>();

        let eligible_nodes_for_role_and_group =
            list_eligible_nodes_for_role_and_group(&eligible_nodes);

        // Check number of returned groups matches what was provided
        let input_group_count: usize = eligible_nodes
            .values()
            .into_iter()
            .map(|group| group.keys().len())
            .sum();

        assert_eq!(input_group_count, eligible_nodes_for_role_and_group.len());

        // test expected outcome
        for (role, group_and_nodes) in &eligible_node_names_parsed {
            for (group, test_nodes) in group_and_nodes {
                let test_labels = get_role_and_group_labels(&role, &group);
                // find the corresponding nodes via labels
                for (eligible_nodes, labels) in &eligible_nodes_for_role_and_group {
                    if test_labels == *labels {
                        // we found the corresponding nodes here, now we check if the size is correct
                        assert_eq!(test_nodes.len(), eligible_nodes.len());
                        // check if the correct nodes are in place
                        for node_name in test_nodes {
                            // create node and check if its contained in eligible nodes
                            let mut node = Node::default();
                            node.metadata.name = Some(node_name.clone());
                            assert!(eligible_nodes.contains(&node));
                        }
                    }
                }
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
