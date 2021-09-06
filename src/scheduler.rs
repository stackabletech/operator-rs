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
        unscheduled_pods: Vec<NodeIdentity>,
    },
}

pub type SchedulerResult<T> = std::result::Result<T, Error>;

pub trait Scheduler<T: PodIdentityGenerator> {
    fn schedule(
        &self,
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
                .unwrap_or("<no-nodename-set>".to_string()),
        }
    }
}

pub struct StickyScheduler {
    pub history: Option<SimpleSchedulerHistory>,
    pub strategy: ScheduleStrategy,
}

pub enum ScheduleStrategy {
    GroupAntiAffinity,
}

impl StickyScheduler {
    pub fn new(history: Option<SimpleSchedulerHistory>, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
    }
}

impl<T> Scheduler<T> for StickyScheduler
where
    T: PodIdentityGenerator,
{
    fn schedule(
        &self,
        // TODO: probably can move to "self"
        id_generator: &T,
        matching_nodes: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<PodToNodeMapping> {
        let mut unscheduled_pods = vec![];
        let mut result = BTreeMap::new();
        let mut matching_nodes_cloned = matching_nodes.clone();
        // generate ids
        let pod_ids = id_generator.generate();

        for pod_id in &pod_ids {
            // pod id not found in current setting
            if !current_mapping.mapping.contains_key(pod_id) {
                // check if pod id can be found in history
                if let Some(node_id) = self
                    .history
                    .as_ref()
                    .and_then(|history| history.history.mapping.get(pod_id))
                {
                    // pod id available in history
                    if let Some(nodes) = matching_nodes_cloned
                        .get(&pod_id.role)
                        .as_mut()
                        .and_then(|role| role.get(&pod_id.group).as_mut())
                    {
                        // node still existing -> return pod_id and assigned node found history
                        if nodes.contains(node_id) {
                            result.insert(pod_id.clone(), node_id.clone());
                        }
                        // node offline / deleted / changed labels
                        else {
                            // if no nodes are available collect unscheduled pods for later error handling
                            if nodes.is_empty() {
                                unscheduled_pods.push(pod_id);
                            // if nodes still available select first node and assign pod_id to selected node
                            } else {
                                // unwrap is safe here.
                                result.insert(pod_id.clone(), nodes.pop().unwrap());
                            }
                        }
                    }
                }
                // pod id not found in history ->  schedule to first node
                else {
                    if let Some(nodes) = &mut matching_nodes_cloned
                        .get(&pod_id.role)
                        .and_then(|role| role.get(&pod_id.group))
                    {
                        // if no nodes are available collect unscheduled pods for later error handling
                        if nodes.is_empty() {
                            unscheduled_pods.push(pod_id);
                        // if nodes still available select first node and assign pod_id to selected node
                        } else {
                            // unwrap is safe here.
                            result.insert(pod_id.clone(), nodes.pop().unwrap());
                        }
                    }
                }
            }
        }

        // TODO: calculate pod length
        //return Err(Error::NotEnoughNodesAvailable {number_of_nodes: nodes.len(), number_of_pods: 0, unscheduled_pods: })

        Ok(PodToNodeMapping { mapping: result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, TryFutureExt};
    use k8s_openapi::api::core::v1::Node;
    use kube::api::ObjectMeta;
    use rand::prelude::IteratorRandom;
    use rstest::*;

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
