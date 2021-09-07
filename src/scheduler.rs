//! This module provides structs and methods to provide some stateful data in the custom resource
//! status.
//!
//! Node assignments are stored in the status to provide 'sticky' pods and ids for scheduling pods
//! to nodes.
//!
use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};

use k8s_openapi::api::core::v1::Node;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Not enough nodes [{number_of_nodes}] available to schedule pods [{number_of_pods}]. Unscheduled pods: {unscheduled_pods:?}."
    )]
    NotEnoughNodesAvailable {
        number_of_nodes: usize,
        number_of_pods: usize,
        unscheduled_pods: Vec<PodIdentity>,
    },
}

pub type SchedulerResult<T> = std::result::Result<T, Error>;

pub trait Scheduler<T: PodIdentityGenerator> {
    fn schedule(
        &mut self,
        id_generator: &T,
        nodes: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        // current state of the cluster
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<PodToNodeMapping>;
}

pub trait PodIdentityGenerator {
    fn generate(&self) -> Vec<PodIdentity>;
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodToNodeMapping {
    mapping: BTreeMap<PodIdentity, NodeIdentity>,
}

impl PodToNodeMapping {
    pub fn get(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
        self.mapping.get(pod_id)
    }

    pub fn insert(&mut self, pod_id: PodIdentity, node_id: NodeIdentity) -> Option<NodeIdentity> {
        self.mapping.insert(pod_id, node_id)
    }

    pub fn filter(&self, id: &PodIdentity) -> Vec<NodeIdentity> {
        self.mapping
            .iter()
            .filter_map(|(pod_id, node_id)| {
                if pod_id.app == id.app
                    && pod_id.instance == id.instance
                    && pod_id.role == id.role
                    && pod_id.group == id.group
                {
                    Some(node_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSchedulerHistory {
    pub history: PodToNodeMapping,
}

impl SimpleSchedulerHistory {
    pub fn find_node_id(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
        self.history.get(pod_id)
    }

    ///
    /// Add mapping to history if doesn't already exist.
    ///
    pub fn update_mapping(&mut self, pod_id: PodIdentity, node_id: NodeIdentity) {
        if let Some(history_node_id) = self.find_node_id(&pod_id) {
            if *history_node_id != node_id {
                self.history.insert(pod_id, node_id);
            }
        }
    }
}
#[derive(
    Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct PodIdentity {
    pub app: String,
    pub instance: String,
    pub role: String,
    pub group: String,
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub name: String,
}

impl Display for NodeIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<Node> for NodeIdentity {
    fn from(node: Node) -> Self {
        NodeIdentity {
            name: node
                .metadata
                .name
                .unwrap_or_else(|| "<no-nodename-set>".to_string()),
        }
    }
}

pub struct StickyScheduler {
    pub history: SimpleSchedulerHistory,
    pub strategy: ScheduleStrategy,
}

pub enum ScheduleStrategy {
    GroupAntiAffinity,
}

impl StickyScheduler {
    pub fn new(history: SimpleSchedulerHistory, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
    }

    ///
    /// Returns a node that is available for scheduling given `role` and `group`.
    ///
    /// If `opt_node_id` is not `None`, return it *if it exists in the eligible nodes*.
    /// Otherwise, the first node in the corresponding group is returned.
    ///
    /// The returned node is also removed from `eligible_nodes`.
    ///
    fn next_node(
        eligible_nodes: &mut BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        opt_node_id: Option<&NodeIdentity>,
        role: &str,
        group: &str,
    ) -> Option<NodeIdentity> {
        if let Some(nodes) = eligible_nodes
            .get_mut(role)
            .and_then(|role| role.get_mut(group))
        {
            if !nodes.is_empty() {
                if let Some(node_id) = opt_node_id {
                    if let Some(index) = (1..nodes.len())
                        .zip(nodes.iter_mut())
                        .find(|(_, n)| node_id == *n)
                        .map(|(i, _)| i)
                    {
                        nodes.remove(index);
                        return opt_node_id.cloned();
                    }
                }
                return nodes.pop();
            }
        }
        None
    }

    fn node_count(matching_nodes: &BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>) -> usize {
        matching_nodes.values().fold(0, |acc, groups| {
            acc + groups
                .values()
                .fold(0, |gnodes, nodes| gnodes + nodes.len())
        })
    }
}

impl<T> Scheduler<T> for StickyScheduler
where
    T: PodIdentityGenerator,
{
    fn schedule(
        &mut self,
        // TODO: probably can move to "self"
        id_generator: &T,
        matching_nodes: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<PodToNodeMapping> {
        let mut unscheduled_pods = vec![];
        let mut result = BTreeMap::new();
        let mut matching_nodes_cloned = matching_nodes;
        // Need to compute this here because matching_nodes is dropped and
        // matching_nodes_cloned is modified afterwards.
        let number_of_nodes = Self::node_count(&matching_nodes_cloned);

        let pod_ids = id_generator.generate();

        for pod_id in &pod_ids {
            if !current_mapping.mapping.contains_key(pod_id) {
                // The pod with `pod_id` is not scheduled yet so try to find a node for it.
                // Look in the history first.
                let history_node_id = self.history.find_node_id(pod_id);

                // Find a node to schedule on (it might be the node from history)
                if let Some(next_node) = Self::next_node(
                    &mut matching_nodes_cloned,
                    history_node_id,
                    pod_id.role.as_str(),
                    pod_id.group.as_str(),
                ) {
                    // update result mapping
                    result.insert(pod_id.clone(), next_node.clone());
                    // update history
                    self.history.update_mapping(pod_id.clone(), next_node)
                } else {
                    unscheduled_pods.push(pod_id.clone());
                }
            }
        }

        if unscheduled_pods.is_empty() {
            Ok(PodToNodeMapping { mapping: result })
        } else {
            return Err(Error::NotEnoughNodesAvailable {
                number_of_nodes,
                number_of_pods: pod_ids.len(),
                unscheduled_pods,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Node;
    use kube::api::ObjectMeta;
    use rand::prelude::IteratorRandom;
    use rstest::*;

    use super::*;

    const APP_NAME: &str = "app";
    const INSTANCE: &str = "simple";
    const ROLE_1: &str = "role_1";
    const ROLE_2: &str = "role_2";
    const GROUP_1: &str = "group_1";
    const GROUP_2: &str = "group_2";

    const AVAILABLE_NODES: usize = 10;

    struct TestIdGenerator {}

    impl PodIdentityGenerator for TestIdGenerator {
        fn generate(&self) -> Vec<PodIdentity> {
            let mut identities = vec![];
            for id in 1..2 {
                identities.push(PodIdentity {
                    app: APP_NAME.to_string(),
                    instance: INSTANCE.to_string(),
                    role: ROLE_1.to_string(),
                    group: GROUP_1.to_string(),
                    id: id.to_string(),
                })
            }
            for id in 3..5 {
                identities.push(PodIdentity {
                    app: APP_NAME.to_string(),
                    instance: INSTANCE.to_string(),
                    role: ROLE_1.to_string(),
                    group: GROUP_2.to_string(),
                    id: id.to_string(),
                })
            }
            for id in 6..8 {
                identities.push(PodIdentity {
                    app: APP_NAME.to_string(),
                    instance: INSTANCE.to_string(),
                    role: ROLE_2.to_string(),
                    group: GROUP_1.to_string(),
                    id: id.to_string(),
                })
            }
            identities
        }
    }

    fn generate_node_identities(replicas: usize) -> Vec<NodeIdentity> {
        let mut nodes = vec![];
        for replica in 1..replicas {
            nodes.push(NodeIdentity {
                name: format!("node_{}", replica),
            });
        }
        nodes
    }

    fn generate_current_mapping(
        already_mapped: usize,
        pod_ids: &Vec<PodIdentity>,
        node_ids: &Vec<NodeIdentity>,
    ) -> BTreeMap<PodIdentity, NodeIdentity> {
        let mut current_mapping = BTreeMap::new();

        for id in 1..already_mapped {
            current_mapping.insert(
                pod_ids.get(id).unwrap().clone(),
                node_ids.get(id).unwrap().clone(),
            );
        }

        current_mapping
    }

    /// Eligible nodes look  like this:
    ///
    ///     {"role1": {
    ///         "group0": [],
    ///         "group1": [NodeIdentity { name: "node11" }],
    ///         "group2": [NodeIdentity { name: "node21" }, NodeIdentity { name: "node22" }]}}
    ///
    fn fill_eligible_nodes() -> BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> {
        let mut roles = BTreeMap::new();
        let mut groups = BTreeMap::new();
        groups.insert("group0".to_string(), vec![]);
        groups.insert(
            "group1".to_string(),
            vec![NodeIdentity {
                name: "node11".to_string(),
            }],
        );
        groups.insert(
            "group2".to_string(),
            vec![
                NodeIdentity {
                    name: "node21".to_string(),
                },
                NodeIdentity {
                    name: "node22".to_string(),
                },
            ],
        );

        roles.insert("role1".to_string(), groups);

        roles
    }

    #[rstest]
    #[case(None, "", "", None)]
    #[case(None, "does not exist", "group0", None)]
    #[case(None, "role1", "does not exist", None)]
    #[case(Some("node22"), "role1", "group0", None)]
    #[case(Some("node22"), "role1", "group1", Some("node11"))] // node not found, use first!
    #[case(Some("node22"), "role1", "group2", Some("node22"))] // node found, use it!
    fn test_next_node(
        #[case] opt_node_id: Option<&str>,
        #[case] role: &str,
        #[case] group: &str,
        #[case] expected: Option<&str>,
    ) {
        let mut eligible_nodes = fill_eligible_nodes();

        let got = StickyScheduler::next_node(
            &mut eligible_nodes,
            opt_node_id
                .map(|n| NodeIdentity {
                    name: n.to_string(),
                })
                .as_ref(),
            role,
            group,
        );

        //println!("{:?}", eligible_nodes);

        assert_eq!(
            got,
            expected.map(|n| NodeIdentity {
                name: n.to_string(),
            })
        )
    }

    #[test]
    fn test_schedule_no_history() {
        // let nodes = generate_node_identities(10);
        //
        // let id_generator = TestIdGenerator {};
        // let mapping = generate_current_mapping(7, &id_generator.generate(), &nodes);
        //
        // let scheduler = StickyScheduler::new(None, ScheduleStrategy::GroupAntiAffinity);
        // println!("{:?}", scheduler.schedule(&id_generator, &nodes, &mapping));
    }
}
