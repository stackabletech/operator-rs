//!
//! Implements scheduler with memory. Once a Pod with a given identifier is scheduled on a node,
//! it will always be rescheduled to this node as long as it exists.
//!
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Debug, Display, Formatter};

use k8s_openapi::api::core::v1::Node;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::Iter;

#[derive(Debug, thiserror::Error, PartialEq)]
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
    ) -> SchedulerResult<SchedulerState>;
}

pub trait PodIdentityGenerator {
    fn generate(&self) -> Vec<PodIdentity>;
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodToNodeMapping {
    mapping: BTreeMap<PodIdentity, NodeIdentity>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerState {
    current_mapping: PodToNodeMapping,
    new_mapping: PodToNodeMapping,
}

impl SchedulerState {
    pub fn new(current_mapping: PodToNodeMapping, new_mapping: PodToNodeMapping) -> Self {
        SchedulerState {
            current_mapping,
            new_mapping,
        }
    }

    pub fn mapping(&self) -> PodToNodeMapping {
        self.current_mapping.merge(&self.new_mapping)
    }

    pub fn new_mapping(&self) -> PodToNodeMapping {
        self.new_mapping.clone()
    }
}

impl PodToNodeMapping {
    pub fn new() -> Self {
        PodToNodeMapping {
            mapping: BTreeMap::new(),
        }
    }

    pub fn iter(&self) -> Iter<'_, PodIdentity, NodeIdentity> {
        self.mapping.iter()
    }

    pub fn get_filtered(&self, role: &str, group: &str) -> BTreeMap<PodIdentity, NodeIdentity> {
        let mut filtered = BTreeMap::new();
        for (pod_id, node_id) in &self.mapping {
            if &pod_id.role == role && &pod_id.group == group {
                filtered.insert(pod_id.clone(), node_id.clone());
            }
        }
        filtered
    }

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

    pub fn merge(&self, other: &Self) -> Self {
        let mut temp = self.mapping.clone();
        temp.extend(other.clone().mapping);
        PodToNodeMapping { mapping: temp }
    }

    pub fn contains_node(&self, node: &NodeIdentity) -> Option<&PodIdentity> {
        for (pod_id, mapped_node) in self.mapping.iter() {
            if node == mapped_node {
                return Some(pod_id);
            }
        }
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSchedulerHistory {
    pub history: PodToNodeMapping,
}

impl SimpleSchedulerHistory {
    pub fn new() -> Self {
        SimpleSchedulerHistory {
            history: PodToNodeMapping {
                mapping: BTreeMap::new(),
            },
        }
    }

    pub fn find_node_id(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
        self.history.get(pod_id)
    }

    ///
    /// Add mapping to history if doesn't already exist.
    ///
    pub fn update_mapping(&mut self, pod_id: PodIdentity, node_id: &NodeIdentity) {
        if let Some(history_node_id) = self.find_node_id(&pod_id) {
            if history_node_id != node_id {
                self.history.insert(pod_id, node_id.clone());
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

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
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

///
/// A scheduler implementation that remembers where pods were once scheduled (based on
/// their ids) and maps them to the same nodes in the future.
///
pub struct StickyScheduler {
    pub history: SimpleSchedulerHistory,
    pub strategy: ScheduleStrategy,
}

pub enum ScheduleStrategy {
    GroupAntiAffinity,
}

/// Implements scheduler with memory. Once a Pod with a given identifier is scheduled on a node,
/// it will always be rescheduled to this node as long as it exists.
impl StickyScheduler {
    /// The `strategy` parameter is ignored for now and a `GroupAntiAffinity` strategy is implemented
    /// by default.
    pub fn new(history: SimpleSchedulerHistory, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
    }

    ///
    /// Returns a node that is available for scheduling given `role` and `group`.
    ///
    /// If `opt_node_id` is not `None`, return it *if it exists in the eligible nodes*.
    /// Otherwise, the first node in the corresponding group is returned.
    ///
    fn next_node(
        eligible_nodes: &BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        opt_node_id: Option<&NodeIdentity>,
        role: &str,
        group: &str,
    ) -> Option<NodeIdentity> {
        if let Some(nodes) = eligible_nodes.get(role).and_then(|role| role.get(group)) {
            if !nodes.is_empty() {
                if let Some(node_id) = opt_node_id {
                    let tmp = nodes.iter().find(|n| *n == node_id);
                    if tmp.is_some() {
                        return tmp.cloned();
                    }
                }
                return nodes.last().cloned();
            }
        }
        None
    }

    fn remove(nodes: &mut Vec<NodeIdentity>, to_remove: &NodeIdentity) -> Option<NodeIdentity> {
        if let Some(index) = (0..nodes.len())
            .zip(nodes.iter_mut())
            .find(|(_, n)| to_remove == *n)
            .map(|(i, _)| i)
        {
            Some(nodes.remove(index))
        } else {
            None
        }
    }

    fn remove_eligible_node(
        eligible_nodes: &mut BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        to_remove: &NodeIdentity,
        role: &str,
        group: &str,
    ) -> bool {
        if let Some(groups) = eligible_nodes.get_mut(role) {
            if let Some(nodes) = groups.get_mut(group) {
                return Self::remove(nodes, to_remove).is_some();
            }
        }
        false
    }

    ///
    /// Count the total number of unique node identities in the `matching_nodes`
    ///
    fn count_unique_node_ids(
        matching_nodes: &BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
    ) -> usize {
        matching_nodes
            .values()
            .flat_map(|groups| groups.values())
            .flatten()
            .collect::<HashSet<&NodeIdentity>>()
            .len()
    }
}

impl<T> Scheduler<T> for StickyScheduler
where
    T: PodIdentityGenerator,
{
    ///
    /// Given the desired pod ids, the eligible nodes and the current state (which pods are already
    /// scheduled/mapped to nodes), computes a mapping of the remaining desired pods.
    ///
    /// Uses a (currently unbounded) history of mappings to reschedule pods to the same nodes
    /// again, provided the nodes are still eligible. Pods that are successfully mapped to new nodes
    /// are added to the history.
    ///
    /// It doesn't map more than one pod per role+group on the same node. If a pod cannot be mapped
    /// (because not enough nodes available, for example) it returns an error.
    ///
    fn schedule(
        &mut self,
        // TODO: probably can move to "self"
        id_generator: &T,
        matching_nodes: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState> {
        let mut unscheduled_pods = vec![];
        let mut result = BTreeMap::new();
        let mut matching_nodes_cloned = matching_nodes;
        // Need to compute this here because matching_nodes is dropped and
        // matching_nodes_cloned is modified afterwards.
        let number_of_nodes = Self::count_unique_node_ids(&matching_nodes_cloned);

        let pod_ids = id_generator.generate();

        for pod_id in &pod_ids {
            if !current_mapping.mapping.contains_key(pod_id) {
                // The pod with `pod_id` is not scheduled yet so try to find a node for it.
                // Look in the history first.
                let history_node_id = self.history.find_node_id(pod_id);

                // Find a node to schedule on (it might be the node from history)
                while let Some(next_node) = Self::next_node(
                    &matching_nodes_cloned,
                    history_node_id,
                    pod_id.role.as_str(),
                    pod_id.group.as_str(),
                ) {
                    // check that the node is not already in use
                    if current_mapping.contains_node(&next_node).is_some() {
                        // next_node is already in use
                        // remove node from matching_nodes_cloned and loop again
                        Self::remove_eligible_node(
                            &mut matching_nodes_cloned,
                            &next_node,
                            pod_id.role.as_str(),
                            pod_id.group.as_str(),
                        );
                        continue;
                    }
                    // update result mapping
                    result.insert(pod_id.clone(), next_node.clone());
                    // update history
                    self.history.update_mapping(pod_id.clone(), &next_node);
                    // remove node from matching_nodes_cloned because now it's used
                    Self::remove_eligible_node(
                        &mut matching_nodes_cloned,
                        &next_node,
                        pod_id.role.as_str(),
                        pod_id.group.as_str(),
                    );
                    // stop while next_node
                    break;
                }

                if !result.contains_key(pod_id) {
                    unscheduled_pods.push(pod_id.clone());
                }
            }
        }

        if unscheduled_pods.is_empty() {
            Ok(SchedulerState::new(
                current_mapping.clone(),
                PodToNodeMapping { mapping: result },
            ))
        } else {
            Err(Error::NotEnoughNodesAvailable {
                number_of_nodes,
                number_of_pods: pod_ids.len(),
                unscheduled_pods,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::array::IntoIter;
    use std::iter::FromIterator;

    const APP_NAME: &str = "app";
    const INSTANCE: &str = "simple";

    struct TestIdGenerator {
        how_many: usize,
    }

    impl PodIdentityGenerator for TestIdGenerator {
        fn generate(&self) -> Vec<PodIdentity> {
            (0..self.how_many)
                .map(|index| PodIdentity {
                    app: APP_NAME.to_string(),
                    instance: INSTANCE.to_string(),
                    role: format!("ROLE_{}", index % 2).to_string(),
                    group: format!("GROUP_{}", index % 2).to_string(),
                    id: format!("POD_{}", index).to_string(),
                })
                .collect()
        }
    }

    fn generate_available_nodes(
        available_node_count: usize,
    ) -> BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> {
        let mut roles: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> = BTreeMap::new();
        for index in 0..available_node_count {
            let role_name = format!("ROLE_{}", index % 2).to_string();
            let group_name = format!("GROUP_{}", index % 2).to_string();
            let node = NodeIdentity {
                name: format!("NODE_{}", index),
            };
            if let Some(role) = roles.get_mut(&role_name) {
                if let Some(group) = role.get_mut(&group_name) {
                    group.push(node);
                } else {
                    role.insert(group_name, vec![node]);
                }
            } else {
                let mut new_group = BTreeMap::new();
                new_group.insert(group_name, vec![node]);
                roles.insert(role_name, new_group);
            }
        }
        roles
    }

    fn generate_current_mapping(
        scheduled_pods: &Vec<PodIdentity>,
        available_nodes: &BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
    ) -> PodToNodeMapping {
        let mut current_mapping = BTreeMap::new();
        let mut available_nodes_clone = available_nodes.clone();

        for pod_id in scheduled_pods {
            if let Some(role) = available_nodes_clone.get_mut(&pod_id.role.to_string()) {
                if let Some(group) = role.get_mut(&pod_id.group.to_string()) {
                    if !group.is_empty() {
                        current_mapping.insert(pod_id.clone(), group.pop().unwrap().clone());
                    }
                }
            }
        }

        PodToNodeMapping {
            mapping: current_mapping,
        }
    }

    #[rustfmt::skip]
     #[rstest]
     #[case::no_pods_to_schedule( 0, 0, 5, SimpleSchedulerHistory::new(), Ok(SchedulerState::default()))]
     #[case::all_pods_are_scheduled( 3, 3, 5, SimpleSchedulerHistory::new(),
         Ok(SchedulerState {
             current_mapping:
                 PodToNodeMapping {
                     mapping: BTreeMap::from_iter(IntoIter::new([
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_0".to_string() }, NodeIdentity { name: "NODE_4".to_string() }),
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_2".to_string() }, NodeIdentity { name: "NODE_2".to_string() }),
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_1".to_string(), group: "GROUP_1".to_string(), id: "POD_1".to_string() }, NodeIdentity { name: "NODE_3".to_string() })
                 ]))},
             new_mapping: PodToNodeMapping::new() }))]    
     #[case::one_pod_is_scheduled(3, 2, 10, SimpleSchedulerHistory::new(),
        Ok(SchedulerState {
            current_mapping:
                PodToNodeMapping {
                    mapping: BTreeMap::from_iter(IntoIter::new([
                        (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_0".to_string() }, NodeIdentity { name: "NODE_8".to_string() }),
                        (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_1".to_string(), group: "GROUP_1".to_string(), id: "POD_1".to_string() }, NodeIdentity { name: "NODE_9".to_string() }),
                    ]))},
            new_mapping:
                PodToNodeMapping {
                    mapping: BTreeMap::from_iter(IntoIter::new([
                        (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_2".to_string() }, NodeIdentity { name: "NODE_6".to_string() }),
                    ]))},
        }))]
     #[case::one_pod_is_scheduled_on_histoy_node(3, 2, 10,
         SimpleSchedulerHistory {
             history: PodToNodeMapping {
                 mapping: BTreeMap::from_iter(IntoIter::new([(
                     PodIdentity {
                         app: "app".to_string(),
                         instance: "simple".to_string(),
                         role: "ROLE_0".to_string(),
                         group: "GROUP_0".to_string(),
                         id: "POD_2".to_string(),
                     },
                     NodeIdentity {
                         name: "NODE_4".to_string(),
                     },)])),},},
         Ok(SchedulerState {
             current_mapping:
                 PodToNodeMapping {
                     mapping: BTreeMap::from_iter(IntoIter::new([
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_0".to_string() }, NodeIdentity { name: "NODE_8".to_string() }),
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_1".to_string(), group: "GROUP_1".to_string(), id: "POD_1".to_string() }, NodeIdentity { name: "NODE_9".to_string() }),
                     ]))},
             new_mapping:
                 PodToNodeMapping {
                     mapping: BTreeMap::from_iter(IntoIter::new([
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_2".to_string() }, NodeIdentity { name: "NODE_4".to_string() }),
                     ]))},
         }))]
     #[case::one_pod_is_scheduled_histoy_node_does_not_exist(3, 2, 10,
         SimpleSchedulerHistory {
             history: PodToNodeMapping {
                 mapping: BTreeMap::from_iter(IntoIter::new([(
                     PodIdentity {
                         app: "app".to_string(),
                         instance: "simple".to_string(),
                         role: "ROLE_0".to_string(),
                         group: "GROUP_0".to_string(),
                         id: "POD_2".to_string(),
                     },
                     NodeIdentity {
                         name: "NODE_14".to_string(), // <---- does not exist
                     },)])),},},
        Ok(SchedulerState {
             current_mapping:
                 PodToNodeMapping {
                     mapping: BTreeMap::from_iter(IntoIter::new([
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_0".to_string() }, NodeIdentity { name: "NODE_8".to_string() }),
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_1".to_string(), group: "GROUP_1".to_string(), id: "POD_1".to_string() }, NodeIdentity { name: "NODE_9".to_string() }),
                     ]))},
             new_mapping:
                 PodToNodeMapping {
                     mapping: BTreeMap::from_iter(IntoIter::new([
                         (PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "POD_2".to_string() }, NodeIdentity { name: "NODE_6".to_string() }),
                     ]))},
         }))]
     #[case::pod_cannot_be_scheduled( 1, 0, 0, SimpleSchedulerHistory::new(),
         Err(Error::NotEnoughNodesAvailable {
             number_of_nodes: 0,
             number_of_pods: 1,
             unscheduled_pods: vec![
                 PodIdentity {
                     app: "app".to_string(),
                     instance: "simple".to_string(),
                     role: "ROLE_0".to_string(),
                     group: "GROUP_0".to_string(),
                     id: "POD_0".to_string() }] }))]
     fn test_scheduler_sticky_scheduler(
         #[case] wanted_pod_count: usize,
         #[case] scheduled_pods_count: usize,
         #[case] available_node_count: usize,
         #[case] history: SimpleSchedulerHistory,
         #[case] expected: SchedulerResult<SchedulerState>,
     ) {
         let id_generator = TestIdGenerator {
             how_many: wanted_pod_count,
         };
         let wanted_pods = id_generator.generate();
         let available_nodes = generate_available_nodes(available_node_count);
         let mut scheduled_pods = vec![];
         for i in 0..scheduled_pods_count {
             scheduled_pods.push(wanted_pods.get(i).unwrap().clone());
         }
         let current_mapping = generate_current_mapping(&scheduled_pods, &available_nodes);
    
         //
         // Run scheduler
         //
         let mut scheduler = StickyScheduler::new(history, ScheduleStrategy::GroupAntiAffinity);
         let got = scheduler.schedule(&id_generator, available_nodes, &current_mapping);
    
         assert_eq!(expected, got);
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
    fn test_scheduler_next_node(
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

        assert_eq!(
            got,
            expected.map(|n| NodeIdentity {
                name: n.to_string(),
            })
        )
    }

    #[test]
    fn test_scheduler_count_unique_node_ids_null() {
        assert_eq!(0, StickyScheduler::count_unique_node_ids(&BTreeMap::new()));
    }

    #[test]
    fn test_scheduler_count_unique_node_ids() {
        let mut roles = BTreeMap::new();
        let mut groups = BTreeMap::new();
        groups.insert("group0".to_string(), vec![]);
        groups.insert(
            "group1".to_string(),
            vec![
                NodeIdentity {
                    name: "node11".to_string(),
                },
                NodeIdentity {
                    name: "node21".to_string(), // duplicate!
                },
            ],
        );
        groups.insert(
            "group2".to_string(),
            vec![
                NodeIdentity {
                    name: "node21".to_string(), // duplicate!
                },
                NodeIdentity {
                    name: "node22".to_string(),
                },
            ],
        );

        roles.insert("role1".to_string(), groups);

        assert_eq!(3, StickyScheduler::count_unique_node_ids(&roles));
    }

    #[rstest]
    #[case(&mut vec![], NodeIdentity{name: "node1".to_string()}, None)]
    #[case(&mut vec![NodeIdentity{name: "node1".to_string()}], NodeIdentity{name: "node1".to_string()}, Some(NodeIdentity{name: "node1".to_string()}))]
    fn test_scheduler_remove(
        #[case] nodes: &mut Vec<NodeIdentity>,
        #[case] to_remove: NodeIdentity,
        #[case] expected: Option<NodeIdentity>,
    ) {
        assert_eq!(StickyScheduler::remove(nodes, &to_remove), expected);
    }
}
