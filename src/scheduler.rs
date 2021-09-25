//!
//! A Kubernetes pod scheduler is responsible for assigning pods to eligible nodes. To achieve this,
//! the scheduler may use different strategies.
//!
//! This module provides traits and implementations for a scheduler with memory called [`StickyScheduler`]
//! and two pod placement strategies : [`ScheduleStrategy::GroupAntiAffinity`] and [`ScheduleStrategy::Hashing`].
//!
//! The former strategy means that no two pods belonging to the same role+group pair may be scheduled on the
//! same node. This is useful for bare metal scenarios.
//!
//! The latter strategy hashes pods to nodes without any regards to the node load.
//!
//! The scheduler implements the idea of "preferred nodes" where pods should be scheduled.
//! Whether a preferred node is selected for a pod depends not only of the node's eligibility but also
//! on the strategy used.
//!
//! One implementation for a preferred nodes provider is the [`K8SUnboundedHistory`] that keeps
//! track of pod placements and reuses the nodes in the future. It uses the K8S resource to store
//! and retrieve past pod to node assignments. The requirement for this preferred node provider is
//! that pod id's are "stable" and have a semantic known to the calling operator.
//!
//!
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::ops::DerefMut;

use kube::api::Resource;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::identity::{NodeIdentity, PodIdentity, PodIdentityFactory, PodToNodeMapping};
use crate::role_utils::EligibleNodesForRoleAndGroup;
use k8s_openapi::api::core::v1::Pod;

pub trait PodPlacementHistory {
    fn find(&self, pod_id: &PodIdentity) -> Option<NodeIdentity>;
    fn find_all(&self, pods: &[PodIdentity]) -> Vec<Option<NodeIdentity>> {
        pods.iter().map(|p| self.find(p)).collect()
    }

    fn update(&mut self, pod_id: &PodIdentity, node_id: &NodeIdentity);
}

pub struct K8SUnboundedHistory<'a> {
    pub client: &'a Client,
    pub history: PodToNodeMapping,
    modified: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoleGroupEligibleNodes {
    node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>>,
}

/// Represents the successful result of a `schedule()`
/// It contains the current scheduled pods and the remaining pods to be scheduled (`remaining_mapping`)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerState {
    current_mapping: PodToNodeMapping,
    remaining_mapping: PodToNodeMapping,
}

pub type SchedulerResult<T> = std::result::Result<T, Error>;

/// Schedule pods to nodes. Implementations might use different strategies to select nodes based on
/// the current mapping of pods to nodes or ignore this completely.
pub trait Scheduler {
    /// Returns the state of the scheduler which describes both the existing mapping as well as
    /// the newly mapped pods.
    ///
    /// Implementations may return an error if not all pods can be mapped to nodes.
    ///
    /// # Arguments:
    /// * `pod_id_factory` - A factory object for all pod identities required by the service.
    /// * `nodes` - Currently available nodes in the system grouped by role and group.
    /// * `pods` - Pods that are already mapped to nodes.
    fn schedule(
        &mut self,
        pod_id_factory: &dyn PodIdentityFactory,
        nodes: &RoleGroupEligibleNodes,
        pods: &[Pod],
    ) -> SchedulerResult<SchedulerState>;
}

pub trait PodPlacementStrategy {
    /// Returns the nodes where each pod should be placed or `None` if the placement for the pod
    /// is not possible.
    /// Assigns `pods` to `NodeIdentities`. For each pod to be placed, if the corresponding
    /// node in `preferred_nodes` is `Some()`, then try to choose this node.
    /// An implementation might still choose a different node if the preferred node contradicts
    /// the implementation strategy.
    /// # Arguments:
    /// * `pods` - A set of pods to assign to nodes.
    /// * `preferred_nodes` - Optional nodes to prioritize during placement (if not None)
    fn place(
        &self,
        pods: &[PodIdentity],
        preferred_nodes: &[Option<NodeIdentity>],
    ) -> Vec<Option<NodeIdentity>>;
}

/// Implements a pod placement strategy where no two pods from the same role+group
/// are scheduled on the same node at the same time. It fails if there are not enough nodes to place pods on.
/// This useful for when pods are deployed on a bare metal K8S environment with Stackable agents as nodes.
struct GroupAntiAffinityStrategy<'a> {
    eligible_nodes: RefCell<RoleGroupEligibleNodes>,
    existing_mapping: &'a PodToNodeMapping,
}

/// Implements a pod placement strategy where pods are hashed to eligible nodes without regards to
/// the existing mapping. This useful for when pods are deployed as containers on a standard K8S
/// environment.
struct HashingStrategy<'a> {
    eligible_nodes: &'a RoleGroupEligibleNodes,
    hasher: RefCell<DefaultHasher>,
}

pub enum ScheduleStrategy {
    /// A scheduling strategy that will refuse to schedule two pods within one role+group on the same
    /// node. If no enough pods are available, the pod will not be scheduled on any node.
    /// This useful for when pods are deployed on a bare metal K8S environment with Stackable agents as nodes.
    GroupAntiAffinity,
    /// A scheduling strategy that will simply hash the pod onto one of the existing nodes without
    /// any consideration for the distribution of all other existing pods.This useful for when pods
    /// are deployed as containers on a standard K8S environment.
    Hashing,
}

/// A scheduler implementation that remembers where pods were once scheduled (based on
/// their ids) and maps them to the same nodes in the future. The `history` provides preferred
/// nodes to map onto based past mappings.
/// The `strategy` might choose a different node if the history node cannot be used.
pub struct StickyScheduler<'a, H: PodPlacementHistory> {
    pub history: &'a mut H,
    pub strategy: ScheduleStrategy,
}

//--------------------------------------------------------------------------------
// Implementation
//--------------------------------------------------------------------------------

impl<'a> K8SUnboundedHistory<'a> {
    pub fn new(client: &'a Client, history: PodToNodeMapping) -> Self {
        K8SUnboundedHistory {
            client,
            history,
            modified: false,
        }
    }

    pub async fn save<T>(&mut self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource,
        <T as Resource>::DynamicType: Default,
    {
        if self.modified {
            return match self
                .client
                .merge_patch_status(resource, &json!({ "history": self.history }))
                .await
            {
                Ok(res) => {
                    self.modified = false;
                    Ok(res)
                }
                err => err,
            };
        }

        Ok(resource.clone())
    }
}

impl SchedulerState {
    pub fn new(current_mapping: PodToNodeMapping, remaining_mapping: PodToNodeMapping) -> Self {
        SchedulerState {
            current_mapping,
            remaining_mapping,
        }
    }

    pub fn mapping(&self) -> PodToNodeMapping {
        self.current_mapping.merge(&self.remaining_mapping)
    }

    pub fn remaining_mapping(&self) -> PodToNodeMapping {
        self.remaining_mapping.clone()
    }
}

impl PodPlacementHistory for K8SUnboundedHistory<'_> {
    fn find(&self, pod_id: &PodIdentity) -> Option<NodeIdentity> {
        self.history.get(pod_id).cloned()
    }

    ///
    /// Add mapping to history if doesn't already exist.
    ///
    fn update(&mut self, pod_id: &PodIdentity, node_id: &NodeIdentity) {
        if let Some(history_node_id) = self.find(pod_id) {
            // found but different
            if history_node_id != *node_id {
                self.history.insert(pod_id.clone(), node_id.clone());
                self.modified = true;
            }
        } else {
            // not found
            self.history.insert(pod_id.clone(), node_id.clone());
            self.modified = true;
        }
    }
}

impl Display for NodeIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Implements scheduler with memory. Once a Pod with a given identifier is scheduled on a node,
/// it will always be rescheduled to this node as long as it exists.
impl<'a, H> StickyScheduler<'a, H>
where
    H: PodPlacementHistory,
{
    pub fn new(history: &'a mut H, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
    }
}

impl<H> Scheduler for StickyScheduler<'_, H>
where
    H: PodPlacementHistory,
{
    /// Returns a state object with pods that need to be scheduled and existing pod mappings.
    ///
    /// Given the desired pod ids, the eligible nodes and the current state (which pods are already
    /// scheduled/mapped to nodes), computes a mapping of the remaining desired pods.
    ///
    /// Uses a (currently unbounded) history of mappings to reschedule pods to the same nodes
    /// again, provided the nodes are still eligible. Pods that are successfully mapped to new nodes
    /// are added to the history.
    ///
    /// The nodes where unscheduled pods are mapped are selected by the configured strategy.
    /// # Arguments:
    /// * `pod_id_factory` - a provider for all pod ides required by the system.
    /// * `nodes` - all eligible nodes available in the system
    /// * `pods` - existing pods that are mapped to nodes.
    fn schedule(
        &mut self,
        pod_id_factory: &dyn PodIdentityFactory,
        nodes: &RoleGroupEligibleNodes,
        pods: &[Pod],
    ) -> SchedulerResult<SchedulerState> {
        let mapping = PodToNodeMapping::try_from(pod_id_factory, pods)?;
        let unscheduled_pods = mapping.missing(pod_id_factory.as_slice());
        let history_nodes = self.history.find_all(unscheduled_pods.as_slice());
        let strategy = self.strategy(nodes, &mapping);
        let selected_nodes = strategy.place(unscheduled_pods.as_slice(), history_nodes.as_slice());

        self.update_history_and_result(
            unscheduled_pods.as_slice(),
            selected_nodes.as_slice(),
            pods.len(),
            nodes.count_unique_node_ids(),
            &mapping,
        )
    }
}

impl<H> StickyScheduler<'_, H>
where
    H: PodPlacementHistory,
{
    fn strategy<'b>(
        &self,
        eligible_nodes: &'b RoleGroupEligibleNodes,
        current_mapping: &'b PodToNodeMapping,
    ) -> Box<dyn PodPlacementStrategy + 'b> {
        match self.strategy {
            ScheduleStrategy::Hashing => Box::new(HashingStrategy::new(eligible_nodes)),
            ScheduleStrategy::GroupAntiAffinity => Box::new(GroupAntiAffinityStrategy::new(
                eligible_nodes.clone(),
                current_mapping,
            )),
        }
    }

    /// Returns the new pod mapping or an error if not all desired pods could be mapped.
    /// As a side effect, it updates the scheduler history.
    /// # Arguments
    /// * `pods` - pods that are not scheduled yet
    /// * `nodes` - the nodes where the yet unscheduled pods would be scheduled on
    /// * `number_of_pods` - count of all pods required by the service
    /// * `number_of_nodes` - count of all nodes available to the system
    /// * `current_mapping` - existing pod to node mapping
    fn update_history_and_result(
        &mut self,
        pods: &[PodIdentity],
        nodes: &[Option<NodeIdentity>],
        number_of_pods: usize,
        number_of_nodes: usize,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState> {
        assert_eq!(pods.len(), nodes.len());
        let mut result_err = vec![];
        let mut result_ok = BTreeMap::new();

        for (pod, opt_node) in pods.iter().zip(nodes) {
            match opt_node {
                Some(node) => {
                    // Found a node to schedule on so update the result
                    result_ok.insert((*pod).clone(), node.clone());
                    // and update the history if needed.
                    self.history.update(pod, node);
                }
                None => result_err.push(format!("{:?}", (*pod).clone())), // No node available for this pod
            }
        }

        if result_err.is_empty() {
            Ok(SchedulerState::new(
                current_mapping.clone(),
                PodToNodeMapping { mapping: result_ok },
            ))
        } else {
            Err(Error::NotEnoughNodesAvailable {
                number_of_nodes,
                number_of_pods,
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
                temp.insert(
                    group_name.clone(),
                    group_nodes
                        .nodes
                        .iter()
                        .map(|n| NodeIdentity::from(n.clone()))
                        .collect(),
                );
            }
            node_set.insert(role_name.clone(), temp);
        }
        RoleGroupEligibleNodes { node_set }
    }

    /// Returns a node that is available for scheduling the given `pod`.
    ///
    /// If `preferred` is `Some` and if it exists in the eligible nodes, return it.
    /// Otherwise, `default` is called with the given pod and a Vec of eligible nodes for the
    /// pod's role and group.
    /// # Arguments:
    /// * `pod` - role name with eligible nodes.
    /// * `preferred` - preferred eligible node to schedule on.
    /// * `default` - a function to select a node for the given pod.
    fn preferred_node_or<F>(
        &self,
        pod: &PodIdentity,
        preferred: Option<NodeIdentity>,
        default: F,
    ) -> Option<NodeIdentity>
    where
        F: Fn(&PodIdentity, &Vec<NodeIdentity>) -> Option<NodeIdentity>,
    {
        match self
            .node_set
            .get(&pod.role().to_string())
            .and_then(|role| role.get(&pod.group().to_string()))
        {
            Some(nodes) if !nodes.is_empty() => {
                if let Some(node_id) = preferred {
                    let tmp = nodes.iter().find(|n| n == &&node_id);
                    if tmp.is_some() {
                        return tmp.cloned();
                    }
                }
                default(pod, nodes)
            }
            _ => None,
        }
    }

    /// Wrapper around [`RoleGroupEligibleNodes::preferred_node_or`] where the `default` is `Vec::last`
    fn preferred_node_or_last(
        &self,
        pod: &PodIdentity,
        preferred: Option<NodeIdentity>,
    ) -> Option<NodeIdentity> {
        self.preferred_node_or(pod, preferred, |_pod, nodes| nodes.last().cloned())
    }

    pub fn remove_eligible_node(&mut self, to_remove: &NodeIdentity, role: &str, group: &str) {
        if let Some(groups) = self.node_set.get_mut(role) {
            if let Some(nodes) = groups.get_mut(group) {
                nodes.retain(|n| n != to_remove);
            }
        }
    }

    ///
    /// Count the total number of unique node identities.
    ///
    pub fn count_unique_node_ids(&self) -> usize {
        self.node_set
            .values()
            .flat_map(|groups| groups.values())
            .flatten()
            .collect::<HashSet<&NodeIdentity>>()
            .len()
    }
}

impl<'a> GroupAntiAffinityStrategy<'a> {
    pub fn new(eligible_nodes: RoleGroupEligibleNodes, pod_node_map: &'a PodToNodeMapping) -> Self {
        GroupAntiAffinityStrategy {
            eligible_nodes: RefCell::new(eligible_nodes),
            existing_mapping: pod_node_map,
        }
    }

    pub fn select_node_for_pod(
        &self,
        pod_id: &PodIdentity,
        preferred_node: Option<NodeIdentity>,
    ) -> Option<NodeIdentity> {
        let mut borrowed_nodes = self.eligible_nodes.borrow_mut();

        // Find a node to schedule on (it might be the node from history)
        while let Some(next_node) =
            borrowed_nodes.preferred_node_or_last(pod_id, preferred_node.clone())
        {
            borrowed_nodes.remove_eligible_node(&next_node, pod_id.role(), pod_id.group());

            // check that the node is not already in use *by a pod from the same role+group*
            if !self
                .existing_mapping
                .mapped_by(&next_node, pod_id.role(), pod_id.group())
            {
                return Some(next_node);
            }
        }
        None
    }
}

impl PodPlacementStrategy for GroupAntiAffinityStrategy<'_> {
    /// Returns a list of nodes to place to provided pods.
    /// *Note* Do not call this more than once! This modifies the internal state of the value that
    /// might not reflect the reality between calls.
    fn place(
        &self,
        pod_identities: &[PodIdentity],
        preferred_nodes: &[Option<NodeIdentity>],
    ) -> Vec<Option<NodeIdentity>> {
        assert_eq!(pod_identities.len(), preferred_nodes.len());
        pod_identities
            .iter()
            .zip(preferred_nodes.iter())
            .map(|(pod, preferred_node)| self.select_node_for_pod(pod, preferred_node.clone()))
            .collect()
    }
}

impl<'a> HashingStrategy<'a> {
    pub fn new(eligible_nodes: &'a RoleGroupEligibleNodes) -> Self {
        Self {
            eligible_nodes,
            hasher: RefCell::new(DefaultHasher::new()),
        }
    }

    fn select_node_for_pod(
        &self,
        pod: &PodIdentity,
        preferred_node: Option<NodeIdentity>,
    ) -> Option<NodeIdentity> {
        self.eligible_nodes
            .preferred_node_or(pod, preferred_node, |pod, nodes| {
                let index =
                    pod.compute_hash(self.hasher.borrow_mut().deref_mut()) as usize % nodes.len();
                nodes.get(index).cloned()
            })
    }
}

impl PodPlacementStrategy for HashingStrategy<'_> {
    fn place(
        &self,
        pods: &[PodIdentity],
        preferred_nodes: &[Option<NodeIdentity>],
    ) -> Vec<Option<NodeIdentity>> {
        assert_eq!(pods.len(), preferred_nodes.len());
        pods.iter()
            .zip(preferred_nodes.iter())
            .map(|(pod, preferred_node)| self.select_node_for_pod(pod, preferred_node.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use crate::identity;
    //use crate::identity::tests;

    use super::*;

    #[derive(Default)]
    struct TestHistory {}

    impl PodPlacementHistory for TestHistory {
        fn find(&self, _pod_id: &PodIdentity) -> Option<NodeIdentity> {
            None
        }

        fn update(&mut self, _pod_id: &PodIdentity, _node_id: &NodeIdentity) {
            // dummy
        }
    }

    #[rstest]
    #[case(vec![], vec![], vec![], vec![], vec![])]
    #[case::place_one_pod_id(
        vec![("master", "default", vec!["node_1"])],
        vec![],
        vec![PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10")],
        vec![None],
        vec![Some(NodeIdentity::new("node_1"))])]
    #[case::place_two_pods_on_the_same_node(
        vec![("master", "default", vec!["node_1"]), ("worker", "default", vec!["node_1"])],
        vec![],
        vec![PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10"), PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "worker", "default", "11")],
        vec![None, None],
        vec![Some(NodeIdentity::new("node_1")), Some(NodeIdentity::new("node_1"))])]
    #[case::place_five_pods_on_three_nodes(
        vec![
            ("master", "default", vec!["node_1", "node_2", "node_3"]),
            ("worker", "default", vec!["node_1", "node_2", "node_3"]),
            ("history", "default", vec!["node_1", "node_2", "node_3"]),],
        vec![],
        vec![
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10"),
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "11"),
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "worker", "default", "12"),
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "worker", "default", "13"),
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "worker", "default", "14"),],
        vec![None, None, None, None, None],
        vec![
            Some(NodeIdentity::new("node_3")),
            Some(NodeIdentity::new("node_2")),
            Some(NodeIdentity::new("node_3")),
            Some(NodeIdentity::new("node_2")),
            Some(NodeIdentity::new("node_1")),
        ])]
    #[case::no_node_to_place_pod(
        vec![],
        vec![],
        vec![PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10")],
        vec![None],
        vec![None])]
    #[case::not_enough_nodes_for_two_pods(
        vec![
            ("master", "default", vec!["node_1"]),
        ],
        vec![],
        vec![
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10"),
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "11"),],
        vec![None, None],
        vec![Some(NodeIdentity::new("node_1")), None])]
    #[case::current_mapping_occupies_node(
        vec![
            ("master", "default", vec!["node_1"]),
        ],
        vec![("node_1", identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10")],
        vec![
            PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "11"),],
        vec![None],
        vec![None])]
    #[case::place_pod_on_preferred_node(
        vec![("master", "default", vec!["node_1", "node_2", "node_3", "node_4"]),],
        vec![],
        vec![PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "master", "default", "10"),],
        vec![Some(NodeIdentity::new("node_2"))],
        vec![Some(NodeIdentity::new("node_2"))])]
    fn test_scheduler_group_anti_affinity(
        #[case] role_group_nodes: Vec<(&str, &str, Vec<&str>)>,
        #[case] current_mapping_node_pod_id: Vec<(&str, &str, &str, &str, &str, &str)>,
        #[case] pod_identities: Vec<PodIdentity>,
        #[case] preferred_nodes: Vec<Option<NodeIdentity>>,
        #[case] expected: Vec<Option<NodeIdentity>>,
    ) {
        let current_mapping = identity::tests::build_mapping(current_mapping_node_pod_id);
        let nodes = build_role_group_nodes(role_group_nodes);

        let strategy = GroupAntiAffinityStrategy::new(nodes, &current_mapping);

        let got = strategy.place(pod_identities.as_slice(), preferred_nodes.as_slice());

        assert_eq!(got, expected.to_vec());
    }

    #[rstest]
    #[case(0, vec![],)]
    #[case(2, vec![
        ("role1", "group1", vec!["node_1", "node_2"]),
        ("role1", "group2", vec!["node_1"]),
    ],)]
    #[case(4, vec![("role1", "group1", vec!["node_1", "node_2", "node_3", "node_4"]),],)]
    fn test_scheduler_count_unique_node_ids(
        #[case] expected: usize,
        #[case] role_group_nodes: Vec<(&str, &str, Vec<&str>)>,
    ) {
        let nodes = build_role_group_nodes(role_group_nodes);
        assert_eq!(expected, nodes.count_unique_node_ids());
    }

    #[rstest]
    #[case(None, PodIdentity::default(), None, vec![])]
    #[case::last(
        Some(NodeIdentity::new("node_3")),
        PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "role1", "group1", "1"),
        None,
        vec![("role1", "group1", vec!["node_1", "node_2", "node_3"])])]
    #[case::preferred(
        Some(NodeIdentity::new("node_2")),
        PodIdentity::new(identity::tests::APP_NAME, identity::tests::INSTANCE, "role1", "group1", "1"),
        Some(NodeIdentity::new("node_2")),
        vec![("role1", "group1", vec!["node_1", "node_2", "node_3"])])]
    fn test_scheduler_preferred_node_or_last(
        #[case] expected: Option<NodeIdentity>,
        #[case] pod_id: PodIdentity,
        #[case] preferred: Option<NodeIdentity>,
        #[case] role_group_nodes: Vec<(&str, &str, Vec<&str>)>,
    ) {
        let nodes = build_role_group_nodes(role_group_nodes);
        let got = nodes.preferred_node_or_last(&pod_id, preferred);
        assert_eq!(got, expected);
    }

    fn build_role_group_nodes(
        eligible_nodes: Vec<(&str, &str, Vec<&str>)>,
    ) -> RoleGroupEligibleNodes {
        let mut node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> = BTreeMap::new();
        for (role, group, node_names) in eligible_nodes {
            node_set
                .entry(String::from(role))
                .and_modify(|r| {
                    r.insert(
                        String::from(group),
                        node_names
                            .iter()
                            .map(|nn| NodeIdentity {
                                name: nn.to_string(),
                            })
                            .collect(),
                    );
                })
                .or_insert_with(|| {
                    vec![(
                        String::from(group),
                        node_names
                            .iter()
                            .map(|nn| NodeIdentity {
                                name: nn.to_string(),
                            })
                            .collect(),
                    )]
                    .into_iter()
                    .collect()
                });
        }
        RoleGroupEligibleNodes { node_set }
    }
}
