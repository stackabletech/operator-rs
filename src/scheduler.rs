//!
//! Implements scheduler with memory. Once a Pod with a given identifier is scheduled on a node,
//! it will always be rescheduled to this node as long as it exists.
//!
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Debug, Display, Formatter};

use k8s_openapi::api::core::v1::{Node, Pod};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::Iter;
use crate::labels;
use crate::role_utils::EligibleNodesForRoleAndGroup;

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

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodToNodeMapping {
    mapping: BTreeMap<PodIdentity, NodeIdentity>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSchedulerHistory {
    pub history: PodToNodeMapping,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoleGroupEligibleNodes {
    node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
}

/// Represents the successful result of a `schedule()`
/// It contains the current scheduled pods and the remaining pods to be scheduled (`new_mapping`)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerState {
    current_mapping: PodToNodeMapping,
    new_mapping: PodToNodeMapping,
}

pub type SchedulerResult<T> = std::result::Result<T, Error>;

/// Schedule pods to nodes. The only implementation available at the moment is the `StickyScheduler`
pub trait Scheduler<T: PodIdentityGenerator> {
    fn schedule(
        &mut self,
        id_generator: &T,
        nodes: &RoleGroupEligibleNodes,
        // current state of the cluster
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState>;
}

pub trait PodIdentityGenerator {
    fn generate(&self) -> Vec<PodIdentity>;
}

/// Implements a pod placement strategy where no two pods from the same role+group
/// are scheduled on the same node at the same time.
/// It fails if there are not enough nodes to place pods on.
/// *Important*: values of this struct are *not reusable* across `schedule()` calls
/// because the `eligible_nodes` is mutated for every successful pod placement.
pub struct GroupAntiAffinityStrategy<'a> {
    eligible_nodes: RoleGroupEligibleNodes,
    pod_node_map: &'a PodToNodeMapping,
}

pub enum ScheduleStrategy {
    GroupAntiAffinity,
}

///
/// A scheduler implementation that remembers where pods were once scheduled (based on
/// their ids) and maps them to the same nodes in the future.
///
pub struct StickyScheduler {
    pub history: SimpleSchedulerHistory,
    pub strategy: ScheduleStrategy,
}

//--------------------------------------------------------------------------------
// Implementation
//--------------------------------------------------------------------------------

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

    pub fn from(pods: &Vec<Pod>, id_label_name: Option<&str>) -> Self {
        let mut pod_node_mapping = PodToNodeMapping::new();
        for pod in pods {
            let labels = &pod.metadata.labels;
            let app = labels.get(labels::APP_NAME_LABEL);
            let instance = labels.get(labels::APP_INSTANCE_LABEL);
            let role = labels.get(labels::APP_COMPONENT_LABEL);
            let group = labels.get(labels::APP_ROLE_GROUP_LABEL);
            let id = id_label_name.and_then(|n| labels.get(n));
            pod_node_mapping.insert(
                PodIdentity {
                    app: app.map(|s| s.clone()).unwrap_or_default(),
                    instance: instance.map(|s| s.clone()).unwrap_or_default(),
                    role: role.map(|s| s.clone()).unwrap_or_default(),
                    group: group.map(|s| s.clone()).unwrap_or_default(),
                    id: id.map(|s| s.clone()).unwrap_or_default(),
                },
                NodeIdentity {
                    name: pod.spec.as_ref().unwrap().node_name.as_ref().unwrap().to_string()
                }
            );
        }
        pod_node_mapping
    }

    pub fn iter(&self) -> Iter<'_, PodIdentity, NodeIdentity> {
        self.mapping.iter()
    }

    pub fn get_filtered(&self, role: &str, group: &str) -> BTreeMap<PodIdentity, NodeIdentity> {
        let mut filtered = BTreeMap::new();
        for (pod_id, node_id) in &self.mapping {
            if pod_id.role == *role && pod_id.group == *group {
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


/// Implements scheduler with memory. Once a Pod with a given identifier is scheduled on a node,
/// it will always be rescheduled to this node as long as it exists.
impl StickyScheduler {
    pub fn new(history: SimpleSchedulerHistory, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
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
        eligible_nodes: &RoleGroupEligibleNodes,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState> {
        let mut result_err = vec![];
        let mut result_ok = BTreeMap::new();

        let mut strategy = match self.strategy {
           ScheduleStrategy::GroupAntiAffinity => GroupAntiAffinityStrategy::new(eligible_nodes.clone(), current_mapping)
        };

       let pod_ids = id_generator.generate();
       for pod_id in pod_ids.iter() {
           if current_mapping.get(pod_id).is_none() {
               // This pod_id is not mapped yet so search for a node where this pod has previously been mapped to.
               // First, lookup a node that has been used in the past (if any).
               let history_node_id = self.history.find_node_id(pod_id);
               // Find a node to schedule on. This might be the node from history, a new one or None.
               let selected_node = strategy.select_node_for_pod(pod_id, history_node_id);
               match selected_node {
                   Some(next_node) => {
                       // Found a node to schedule on so update the result
                       result_ok.insert(pod_id.clone(), next_node.clone());
                       // and update the history if needed.
                       self.history.update_mapping(pod_id.clone(), &next_node);
                   },
                   None => result_err.push(pod_id.clone()), // No node available for this pod
               }
           }
       }

        if result_err.is_empty() {
            Ok(SchedulerState::new(
                current_mapping.clone(),
                PodToNodeMapping { mapping: result_ok },
            ))
        } else {
            Err(Error::NotEnoughNodesAvailable {
                number_of_nodes: eligible_nodes.count_unique_node_ids(),
                number_of_pods: pod_ids.len(),
                unscheduled_pods: result_err,
            })
        }
    }
}

impl RoleGroupEligibleNodes {

    pub fn from(nodes: &EligibleNodesForRoleAndGroup) -> Self {
        let mut node_set = BTreeMap::new();
        for (role_name, group) in nodes {
            let mut temp = BTreeMap::new();
            for (group_name, group_nodes) in group {
                temp.insert(group_name.clone(), group_nodes.nodes.iter().map(|n| NodeIdentity::from(n.clone())).collect());
            }
            node_set.insert(role_name.clone(), temp);
        }
        RoleGroupEligibleNodes { node_set }
    }

    ///
    /// Returns a node that is available for scheduling given `role` and `group`.
    ///
    /// If `opt_node_id` is not `None`, return it *if it exists in the eligible nodes*.
    /// Otherwise, the first node in the corresponding group is returned.
    ///
    pub fn next_node(&self,
                 preferred_node: Option<&NodeIdentity>,
                 role: &str,
                 group: &str,
    ) -> Option<NodeIdentity> {
        if let Some(nodes) = self.node_set.get(role).and_then(|role| role.get(group)) {
            if !nodes.is_empty() {
                if let Some(node_id) = preferred_node {
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

    pub fn remove_eligible_node(
        &mut self,
        to_remove: &NodeIdentity,
        role: &str,
        group: &str,
    ) {
        if let Some(groups) = self.node_set.get_mut(role) {
            if let Some(nodes) = groups.get_mut(group) {
                nodes.retain(|n| n != to_remove);
            }
        }
    }

    ///
    /// Count the total number of unique node identities in the `matching_nodes`
    ///
    pub fn count_unique_node_ids(&self) -> usize {
        self.node_set
            .values()
            .flat_map(|groups| groups.values())
            .flatten()
            .collect::<HashSet<&NodeIdentity>>()
            .len()
    }

    #[cfg(test)]
    fn get_nodes_mut(&mut self, role: &String, group: &String) -> Option<&mut Vec<NodeIdentity>> {
       self.node_set.get_mut(role).and_then(|g| g.get_mut(group))
    }
}


impl <'a> GroupAntiAffinityStrategy<'a> {
    pub fn new(
        eligible_nodes: RoleGroupEligibleNodes,
        pod_node_map: &'a PodToNodeMapping,
    ) -> Self {
        GroupAntiAffinityStrategy {
            eligible_nodes,
            pod_node_map,
        }
    }

    pub fn select_node_for_pod(&mut self, pod_id: &PodIdentity, preferred_node: Option<&NodeIdentity>) -> Option<NodeIdentity> {
        // Find a node to schedule on (it might be the node from history)
        while let Some(next_node) = self.eligible_nodes.next_node(
            preferred_node,
            pod_id.role.as_str(),
            pod_id.group.as_str(),
        ) {
            // check that the node is not already in use
            if self.pod_node_map.contains_node(&next_node).is_some() {
                // next_node is already in use
                // remove node from matching_nodes_cloned and loop again
                self.eligible_nodes.remove_eligible_node(
                    &next_node,
                    pod_id.role.as_str(),
                    pod_id.group.as_str(),
                );
            }
            else {
                return Some(next_node);
            }
        }
        None
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

    fn generate_eligible_nodes(
        available_node_count: usize,
    ) -> RoleGroupEligibleNodes {
        let mut node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> = BTreeMap::new();
        for index in 0..available_node_count {
            let role_name = format!("ROLE_{}", index % 2).to_string();
            let group_name = format!("GROUP_{}", index % 2).to_string();
            let node = NodeIdentity {
                name: format!("NODE_{}", index),
            };
            if let Some(role) = node_set.get_mut(&role_name) {
                if let Some(group) = role.get_mut(&group_name) {
                    group.push(node);
                } else {
                    role.insert(group_name, vec![node]);
                }
            } else {
                let mut new_group = BTreeMap::new();
                new_group.insert(group_name, vec![node]);
                node_set.insert(role_name, new_group);
            }
        }
        RoleGroupEligibleNodes{node_set}
    }

    fn generate_current_mapping(
        scheduled_pods: &Vec<PodIdentity>,
        mut available_nodes: RoleGroupEligibleNodes,
    ) -> PodToNodeMapping {
        let mut current_mapping = BTreeMap::new();

        for pod_id in scheduled_pods {
            let nodes = available_nodes.get_nodes_mut(&pod_id.role, &pod_id.group).unwrap();
            current_mapping.insert(pod_id.clone(), nodes.pop().unwrap().clone());
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
         let available_nodes = generate_eligible_nodes(available_node_count);
         let scheduled_pods = wanted_pods.iter().take(scheduled_pods_count).map(|p| p.clone()).collect();
         let current_mapping = generate_current_mapping(&scheduled_pods, available_nodes.clone());
    
         //
         // Run scheduler
         //
         let mut scheduler = StickyScheduler::new(history, ScheduleStrategy::GroupAntiAffinity);
         let got = scheduler.schedule(&id_generator, &available_nodes, &current_mapping);
    
         assert_eq!(expected, got);
     }

    #[rstest]
    #[case(1, None, "", "", None)]
    #[case(0, Some(NodeIdentity{name: "NODE_2".to_string()}), "ROLE_0", "GROUP_0", None)]
    #[case(3, Some(NodeIdentity{name: "NODE_2".to_string()}), "ROLE_1", "GROUP_1", Some(NodeIdentity{name: "NODE_1".to_string()}))] // node not found, use first!
    #[case(4, Some(NodeIdentity{name: "NODE_2".to_string()}), "ROLE_0", "GROUP_0", Some(NodeIdentity{name: "NODE_2".to_string()}))] // node found, use it!
    fn test_scheduler_group_antiaffinity_next_node(
        #[case] eligible_node_count: usize,
        #[case] opt_node_id: Option<NodeIdentity>,
        #[case] role: &str,
        #[case] group: &str,
        #[case] expected: Option<NodeIdentity>,
    ) {
        let eligible_nodes = generate_eligible_nodes(eligible_node_count);

        let got = eligible_nodes.next_node(opt_node_id.as_ref(), role, group);

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(3, 3)]
    fn test_scheduler_count_unique_node_ids(
        #[case] eligible_node_count: usize,
        #[case] expected: usize,
    ) {
        let eligible_nodes = generate_eligible_nodes(eligible_node_count);
        assert_eq!(expected, eligible_nodes.count_unique_node_ids());
    }
}
