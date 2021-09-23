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
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};

use crate::client::Client;
use crate::error::OperatorResult;
use crate::labels;
use crate::role_utils::EligibleNodesForRoleAndGroup;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::api::Resource;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use std::collections::btree_map::Iter;
use std::collections::hash_map::DefaultHasher;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use std::ops::DerefMut;
use tracing::{error, warn};

const DEFAULT_NODE_NAME: &str = "<no-nodename-set>";
const SEMICOLON: &str = ";";

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

    #[error("PodIdentity could not be parsed: {pod_id:?}. This should not happen. Please open a ticket.")]
    PodIdentityNotParseable { pod_id: String },

    #[error("Cannot build PodIdentity from Pod without labels.")]
    PodWithoutLabelsNotSupported,

    #[error("Cannot build NodeIdentity from node without name.")]
    NodeWithoutNameNotSupported,

    #[error("Cannot construct PodIdentity from empty id field.")]
    PodIdentityFieldEmpty,
}

/// Returns a Vec of pod identities according to the replica per role+group pair from `eligible_nodes`.
///
/// The `id` field is in the range from one (1) to the number of replicas per role+group. If no replicas
/// are defined, then the range goes from one (1) to the number of eligible groups.
///
/// Given a role with two groups, one two replicas and the other with three replicas, the
/// generated `id` fields of the pod identities are counted as follows:
///
/// ```yaml
/// role_1:
///     - group_1:
///         - id: 1
///         - id: 2
///     - group_2:
///         - id: 1
///         - id: 2
///         - id: 3
/// ```
///
/// *NOTE* This function is tightly coupled with [`PodToNodeMapping::try_from_pods`]! If you change it's
/// implementation you also have to update that one.
///
/// # Arguments
/// * `app_name` - Application name
/// * `instance` - Service instance
/// * `eligible_nodes` - Eligible nodes grouped by role and groups.
pub fn generate_ids(
    app_name: &str,
    instance: &str,
    eligible_nodes: &EligibleNodesForRoleAndGroup,
) -> Vec<PodIdentity> {
    let mut generated_ids = vec![];
    for (role_name, groups) in eligible_nodes {
        for (group_name, eligible_nodes) in groups {
            let ids_per_group = eligible_nodes
                .replicas
                .map(usize::from)
                .unwrap_or_else(|| eligible_nodes.nodes.len());
            for id in 1..ids_per_group + 1 {
                generated_ids.push(PodIdentity {
                    app: app_name.to_string(),
                    instance: instance.to_string(),
                    role: role_name.clone(),
                    group: group_name.clone(),
                    id: id.to_string(),
                });
            }
        }
    }

    // Sort the result to make testing predictable. Otherwise the for-loop above is not
    // guaranteed to preserve insertion order so the tests might fail at random.
    generated_ids.sort_by(|a, b| a.partial_cmp(b).unwrap());
    generated_ids
}

#[derive(
    Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub struct PodIdentity {
    app: String,
    instance: String,
    role: String,
    group: String,
    id: String,
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

pub trait PodPlacementHistory {
    fn find(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity>;
    fn find_all(&self, pods: &[&PodIdentity]) -> Vec<Option<&NodeIdentity>> {
        pods.iter().map(|p| self.find(*p)).collect()
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
    /// * `pods` - The list of desired pods. Should contain both already mapped as well as new pods.
    /// * `nodes` - Currently available nodes in the system grouped by role and group.
    /// * `mapped_pods` - Pods that are already mapped to nodes.
    fn schedule(
        &mut self,
        pods: &[PodIdentity],
        nodes: &RoleGroupEligibleNodes,
        mapped_pods: &PodToNodeMapping,
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
        pods: &[&PodIdentity],
        preferred_nodes: &[Option<&NodeIdentity>],
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

impl TryFrom<&Pod> for NodeIdentity {
    type Error = Error;
    fn try_from(p: &Pod) -> SchedulerResult<Self> {
        let node_name = p
            .spec
            .as_ref()
            .map(|s| s.node_name.as_ref())
            .ok_or(Error::NodeWithoutNameNotSupported)?;

        Ok(NodeIdentity {
            name: node_name.unwrap().clone(),
        })
    }
}

impl TryFrom<String> for PodIdentity {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Error> {
        let split = s.split(SEMICOLON).collect::<Vec<&str>>();
        if split.len() != 5 {
            return Err(Error::PodIdentityNotParseable { pod_id: s });
        }
        Ok(PodIdentity::new(
            split[0], split[1], split[2], split[3], split[4],
        ))
    }
}

impl From<PodIdentity> for String {
    fn from(pod_id: PodIdentity) -> Self {
        [
            pod_id.app,
            pod_id.instance,
            pod_id.role,
            pod_id.group,
            pod_id.id,
        ]
        .join(SEMICOLON)
    }
}

impl PodToNodeMapping {
    /// Returns a `PodToNodeMapping` where pod ids are built from (expected) pod labels and self generated counter.
    ///
    /// Returns an `Err` if any of the required labels is not present.
    ///
    /// This is intended for pods that do not maintain state on the nodes where they are scheduled,
    /// such as Spark masters or workers. For ZooKeeper see [`try_from_pods_and_id_label`].
    ///
    /// *NOTE* This function assumes that the operator has also generated id labels by using a counter
    /// starting as one (1) such as [`generate_ids`] in this module. If this was not the case, you
    /// should not use this function to construct [`PodToNodeMapping`]s from pods! Any change here
    /// needs to be implemented in [`generate_ids`] too!
    pub fn try_from_pods(pods: &[Pod]) -> SchedulerResult<PodToNodeMapping> {
        let mut result = PodToNodeMapping::default();
        let mut label_count: HashMap<String, usize> = HashMap::new();
        for p in pods {
            let index = label_count
                .entry(PodIdentity::labels(p)?)
                .and_modify(|e| *e += 1)
                .or_insert(1);
            let index_str = (*index).to_string();
            result.insert(
                PodIdentity::try_from_pod_and_id(p, &index_str)?,
                NodeIdentity::try_from(p)?,
            );
        }
        Ok(result)
    }

    /// Returns a `PodToNodeMapping` where pod ids are built from (expected) pod labels including an id label
    /// that must be present.
    ///
    /// Returns an `Err` if any of the required labels is not present.
    ///
    /// This is intended for pods that explicitly maintain state on the nodes where they are scheduled,
    /// such as ZooKeeper pods. For services where this is not the case, such as Spark masters or workers,
    /// see [`try_from_pods`].
    pub fn try_from_pods_and_id_label(
        pods: &[Pod],
        id_label_name: &str,
    ) -> SchedulerResult<PodToNodeMapping> {
        let mut pod_node_mapping = PodToNodeMapping::default();

        for pod in pods {
            match &pod.metadata.labels {
                Some(labels) => {
                    let id = labels
                        .get(id_label_name)
                        .ok_or(Error::PodWithoutLabelsNotSupported)?;
                    pod_node_mapping.insert(
                        PodIdentity::try_from_pod_and_id(pod, id.as_ref())?,
                        NodeIdentity::try_from(pod)?,
                    );
                }
                None => return Err(Error::PodWithoutLabelsNotSupported),
            }
        }
        Ok(pod_node_mapping)
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

    /// Return true if the `node` is already mapped by pod from `role` and `group`.
    pub fn mapped_by(&self, node: &NodeIdentity, role: &str, group: &str) -> bool {
        for (pod_id, mapped_node) in self.mapping.iter() {
            if node == mapped_node && pod_id.role == role && pod_id.group == group {
                return true;
            }
        }
        false
    }

    /// Given `pods` return all that are not mapped.
    pub fn missing<'a>(&self, pods: &'a [PodIdentity]) -> Vec<&'a PodIdentity> {
        let mut result = vec![];
        for p in pods {
            if !self.mapping.contains_key(p) {
                result.push(p)
            }
        }
        result
    }

    #[cfg(test)]
    pub fn new(map: Vec<(PodIdentity, NodeIdentity)>) -> Self {
        let mut result = BTreeMap::new();
        for (p, n) in map {
            result.insert(p.clone(), n.clone());
        }
        PodToNodeMapping { mapping: result }
    }
}

impl PodPlacementHistory for K8SUnboundedHistory<'_> {
    fn find(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
        self.history.get(pod_id)
    }

    ///
    /// Add mapping to history if doesn't already exist.
    ///
    fn update(&mut self, pod_id: &PodIdentity, node_id: &NodeIdentity) {
        if let Some(history_node_id) = self.find(pod_id) {
            // found but different
            if history_node_id != node_id {
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

impl From<Node> for NodeIdentity {
    fn from(node: Node) -> Self {
        NodeIdentity {
            name: node
                .metadata
                .name
                .unwrap_or_else(|| DEFAULT_NODE_NAME.to_string()),
        }
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
    /// * `pods` - all pod ids required by the service.
    /// * `nodes` - all eligible nodes available in the system
    /// * `mapped_pods` - existing pod to node mapping
    fn schedule(
        &mut self,
        pods: &[PodIdentity],
        nodes: &RoleGroupEligibleNodes,
        mapped_pods: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState> {
        let unscheduled_pods = mapped_pods.missing(pods);
        let history_nodes = self.history.find_all(unscheduled_pods.as_slice());

        let strategy = self.strategy(nodes, mapped_pods);
        let selected_nodes = strategy.place(unscheduled_pods.as_slice(), history_nodes.as_slice());

        self.update_history_and_result(
            unscheduled_pods,
            selected_nodes.to_vec(),
            pods.len(),
            nodes.count_unique_node_ids(),
            mapped_pods,
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
        pods: Vec<&PodIdentity>,
        nodes: Vec<Option<NodeIdentity>>,
        number_of_pods: usize,
        number_of_nodes: usize,
        current_mapping: &PodToNodeMapping,
    ) -> SchedulerResult<SchedulerState> {
        assert_eq!(pods.len(), nodes.len());
        let mut result_err = vec![];
        let mut result_ok = BTreeMap::new();

        for (pod, opt_node) in pods.iter().zip(&nodes) {
            match opt_node {
                Some(node) => {
                    // Found a node to schedule on so update the result
                    result_ok.insert((*pod).clone(), node.clone());
                    // and update the history if needed.
                    self.history.update(pod, node);
                }
                None => result_err.push((*pod).clone()), // No node available for this pod
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
        preferred: Option<&NodeIdentity>,
        default: F,
    ) -> Option<NodeIdentity>
    where
        F: Fn(&PodIdentity, &Vec<NodeIdentity>) -> Option<NodeIdentity>,
    {
        match self
            .node_set
            .get(&pod.role)
            .and_then(|role| role.get(&pod.group))
        {
            Some(nodes) if !nodes.is_empty() => {
                if let Some(node_id) = preferred {
                    let tmp = nodes.iter().find(|n| *n == node_id);
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
        preferred: Option<&NodeIdentity>,
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
    fn get_nodes_mut(&mut self, role: &str, group: &str) -> Option<&mut Vec<NodeIdentity>> {
        self.node_set.get_mut(role).and_then(|g| g.get_mut(group))
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
        preferred_node: Option<&NodeIdentity>,
    ) -> Option<NodeIdentity> {
        let mut borrowed_nodes = self.eligible_nodes.borrow_mut();

        // Find a node to schedule on (it might be the node from history)
        while let Some(next_node) = borrowed_nodes.preferred_node_or_last(pod_id, preferred_node) {
            borrowed_nodes.remove_eligible_node(
                &next_node,
                pod_id.role.as_str(),
                pod_id.group.as_str(),
            );

            // check that the node is not already in use *by a pod from the same role+group*
            if !self.existing_mapping.mapped_by(
                &next_node,
                pod_id.role.as_str(),
                pod_id.group.as_str(),
            ) {
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
        pods: &[&PodIdentity],
        preferred_nodes: &[Option<&NodeIdentity>],
    ) -> Vec<Option<NodeIdentity>> {
        assert_eq!(pods.len(), preferred_nodes.len());
        pods.iter()
            .zip(preferred_nodes.iter())
            .map(|(pod, preferred_node)| self.select_node_for_pod(*pod, *preferred_node))
            .collect()
    }
}

impl PodIdentity {
    pub fn new(app: &str, instance: &str, role: &str, group: &str, id: &str) -> Self {
        Self::warn_forbidden_char(app, instance, role, group, id);
        PodIdentity {
            app: app.to_string(),
            instance: instance.to_string(),
            role: role.to_string(),
            group: group.to_string(),
            id: id.to_string(),
        }
    }

    /// Returns a string with all pod labels required by the [`PodIdentity`] joined with comma.
    pub fn labels(pod: &Pod) -> SchedulerResult<String> {
        if pod.metadata.labels.is_none() {
            return Err(Error::PodWithoutLabelsNotSupported);
        }

        let mut result: Vec<String> = vec![];

        let pod_labels = &pod.metadata.labels.as_ref().unwrap();
        for label_name in [
            labels::APP_NAME_LABEL,
            labels::APP_INSTANCE_LABEL,
            labels::APP_COMPONENT_LABEL,
            labels::APP_ROLE_GROUP_LABEL,
        ] {
            result.push(
                pod_labels
                    .get(label_name)
                    .cloned()
                    .ok_or(Error::PodWithoutLabelsNotSupported)?,
            );
        }
        Ok(result.join(","))
    }

    pub fn try_from_pod_and_id(pod: &Pod, id: &str) -> SchedulerResult<Self> {
        if id.is_empty() {
            return Err(Error::PodIdentityFieldEmpty);
        }

        match &pod.metadata.labels {
            Some(labels) => {
                let app = labels.get(labels::APP_NAME_LABEL);
                let instance = labels.get(labels::APP_INSTANCE_LABEL);
                let role = labels.get(labels::APP_COMPONENT_LABEL);
                let group = labels.get(labels::APP_ROLE_GROUP_LABEL);
                Ok(PodIdentity {
                    app: app.cloned().ok_or(Error::PodWithoutLabelsNotSupported)?,
                    instance: instance
                        .cloned()
                        .ok_or(Error::PodWithoutLabelsNotSupported)?,
                    role: role.cloned().ok_or(Error::PodWithoutLabelsNotSupported)?,
                    group: group.cloned().ok_or(Error::PodWithoutLabelsNotSupported)?,
                    id: id.to_string(),
                })
            }
            _ => Err(Error::PodWithoutLabelsNotSupported),
        }
    }

    pub fn app(&self) -> &str {
        self.app.as_ref()
    }
    pub fn instance(&self) -> &str {
        self.instance.as_ref()
    }
    pub fn role(&self) -> &str {
        self.role.as_ref()
    }
    pub fn group(&self) -> &str {
        self.group.as_ref()
    }
    pub fn id(&self) -> &str {
        self.id.as_ref()
    }

    pub fn compute_hash(&self, hasher: &mut DefaultHasher) -> u64 {
        self.hash(hasher);
        hasher.finish()
    }

    fn warn_forbidden_char(app: &str, instance: &str, role: &str, group: &str, id: &str) {
        if app.contains(SEMICOLON) {
            warn!(
                "Found forbidden character [{}] in application name: {}",
                SEMICOLON, app
            );
        }
        if instance.contains(SEMICOLON) {
            warn!(
                "Found forbidden character [{}] in instance name: {}",
                SEMICOLON, instance
            );
        }
        if role.contains(SEMICOLON) {
            warn!(
                "Found forbidden character [{}] in role name: {}",
                SEMICOLON, role
            );
        }
        if group.contains(SEMICOLON) {
            warn!(
                "Found forbidden character [{}] in group name: {}",
                SEMICOLON, group
            );
        }
        if id.contains(SEMICOLON) {
            warn!(
                "Found forbidden character [{}] in pod id: {}",
                SEMICOLON, id
            );
        }
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
        preferred_node: Option<&NodeIdentity>,
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
        pods: &[&PodIdentity],
        preferred_nodes: &[Option<&NodeIdentity>],
    ) -> Vec<Option<NodeIdentity>> {
        assert_eq!(pods.len(), preferred_nodes.len());
        pods.iter()
            .zip(preferred_nodes.iter())
            .map(|(pod, preferred_node)| self.select_node_for_pod(*pod, *preferred_node))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ObjectMetaBuilder, PodBuilder};
    use crate::role_utils::EligibleNodesAndReplicas;
    use rstest::*;

    const APP_NAME: &str = "app";
    const INSTANCE: &str = "simple";

    #[derive(Default)]
    struct TestHistory {
        pub history: PodToNodeMapping,
    }

    /// A wrapper around `super::generate_ids` that returns pod ids generated by the following scheme:
    /// ```yaml
    /// role_0:
    ///     - group_0:
    ///         - id: 1
    ///     - group_1:
    ///         - id: 1
    ///         - id: 2
    ///         - ... # up to `how_many` - 1
    /// ```
    fn generate_ids(how_many: usize) -> Vec<PodIdentity> {
        if how_many == 0 {
            return vec![];
        }

        let mut eligible_nodes = EligibleNodesForRoleAndGroup::new();
        eligible_nodes.insert(
            "ROLE_0".to_string(),
            [
                (
                    "GROUP_0".to_string(),
                    EligibleNodesAndReplicas {
                        nodes: vec![],
                        replicas: Some(1),
                    },
                ),
                (
                    "GROUP_1".to_string(),
                    EligibleNodesAndReplicas {
                        nodes: vec![],
                        replicas: Some(how_many as u16 - 1),
                    },
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        );
        super::generate_ids(APP_NAME, INSTANCE, &eligible_nodes)
    }

    impl PodPlacementHistory for TestHistory {
        fn find(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
            self.history.get(pod_id)
        }

        fn update(&mut self, _pod_id: &PodIdentity, _node_id: &NodeIdentity) {
            // dummy
        }
    }

    /// Returns nodes generated by the following scheme:
    ///
    /// ```yaml
    /// role_0:
    ///     - group_0:
    ///         - id: 1
    ///     - group_1:
    ///         - id: 1
    ///         - id: 2
    ///         - ... # up to `available_node_count` - 1
    /// ```
    fn generate_eligible_nodes(available_node_count: usize) -> RoleGroupEligibleNodes {
        let mut node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> = BTreeMap::new();
        if available_node_count > 0 {
            let group_1: Vec<NodeIdentity> = (1..available_node_count)
                .map(|i| NodeIdentity {
                    name: format!("NODE_{}", i),
                })
                .collect();
            node_set.insert(
                "ROLE_0".to_string(),
                [
                    (
                        "GROUP_0".to_string(),
                        vec![NodeIdentity {
                            name: "NODE_0".to_string(),
                        }],
                    ),
                    ("GROUP_1".to_string(), group_1),
                ]
                .iter()
                .cloned()
                .collect(),
            );
        }
        RoleGroupEligibleNodes { node_set }
    }

    fn generate_current_mapping(
        scheduled_pods: &[PodIdentity],
        mut available_nodes: RoleGroupEligibleNodes,
    ) -> PodToNodeMapping {
        let mut current_mapping = BTreeMap::new();

        for pod_id in scheduled_pods {
            let nodes = available_nodes
                .get_nodes_mut(&pod_id.role, &pod_id.group)
                .unwrap();
            current_mapping.insert(pod_id.clone(), nodes.pop().unwrap().clone());
        }

        PodToNodeMapping {
            mapping: current_mapping,
        }
    }

    #[rstest]
    #[case::nothing_to_place(1, 1, 1, &[], &[])]
    #[case::not_enough_nodes(1, 0, 0, &[None], &[None])]
    #[case::place_one_pod(1, 0, 1, &[None], &[Some(NodeIdentity { name: "NODE_0".to_string() })])]
    #[case::place_one_pod_on_preferred(2, 1, 5, &[Some(NodeIdentity { name: "NODE_2".to_string() })], &[Some(NodeIdentity { name: "NODE_2".to_string() })])]
    #[case::place_three_pods(3, 0, 5, &[None, None, None],
        &[Some(NodeIdentity { name: "NODE_0".to_string() }),
          Some(NodeIdentity { name: "NODE_4".to_string() }),
          Some(NodeIdentity { name: "NODE_3".to_string() })])]
    #[case::place_three_pods_one_on_preferred(3, 0, 5, &[None, Some(NodeIdentity { name: "NODE_1".to_string() }), None],
        &[Some(NodeIdentity { name: "NODE_0".to_string() }),
          Some(NodeIdentity { name: "NODE_1".to_string() }),
          Some(NodeIdentity { name: "NODE_4".to_string() })])]
    #[case::place_one_pod_when_two_already_mapped(3, 2, 3, &[None],
        &[Some(NodeIdentity { name: "NODE_1".to_string() })])]
    fn test_scheduler_group_anti_affinity(
        #[case] wanted_pod_count: usize,
        #[case] scheduled_pods_count: usize,
        #[case] available_node_count: usize,
        #[case] preferred_nodes: &[Option<NodeIdentity>],
        #[case] expected: &[Option<NodeIdentity>],
    ) {
        let wanted_pods = generate_ids(wanted_pod_count);
        let eligible_nodes = generate_eligible_nodes(available_node_count);

        let scheduled_pods: Vec<_> = wanted_pods
            .iter()
            .take(scheduled_pods_count)
            .cloned()
            .collect();
        let current_mapping = generate_current_mapping(&scheduled_pods, eligible_nodes.clone());

        let vec_preferred_nodes: Vec<Option<&NodeIdentity>> =
            preferred_nodes.iter().map(|o| o.as_ref()).collect();
        let strategy = GroupAntiAffinityStrategy::new(eligible_nodes, &current_mapping);
        let got = strategy.place(
            current_mapping.missing(wanted_pods.as_slice()).as_slice(),
            vec_preferred_nodes.as_slice(),
        );

        assert_eq!(got, expected.to_vec());
    }

    // This is a regression test for a bug that appeared when scheduling a Spark service
    // with 2 masters, 2 workers and 1 history pods on 3 nodes.
    #[test]
    fn test_scheduler_group_anti_affinity_for_spark() {
        let expected = [
            Some(NodeIdentity {
                name: "NODE_0".to_string(),
            }),
            Some(NodeIdentity {
                name: "NODE_1".to_string(),
            }),
            Some(NodeIdentity {
                name: "NODE_0".to_string(),
            }),
        ];

        let nodes = vec![
            NodeIdentity {
                name: "NODE_2".to_string(),
            },
            NodeIdentity {
                name: "NODE_1".to_string(),
            },
            NodeIdentity {
                name: "NODE_0".to_string(),
            },
        ];

        let mut node_set: BTreeMap<String, BTreeMap<String, Vec<NodeIdentity>>> = BTreeMap::new();
        node_set.insert(
            "master".to_string(),
            [("default".to_string(), nodes.clone())]
                .iter()
                .cloned()
                .collect(),
        );
        node_set.insert(
            "worker".to_string(),
            [("default".to_string(), nodes.clone())]
                .iter()
                .cloned()
                .collect(),
        );
        node_set.insert(
            "history".to_string(),
            [("default".to_string(), nodes)].iter().cloned().collect(),
        );
        let eligible_nodes = RoleGroupEligibleNodes { node_set };

        let current_mapping = PodToNodeMapping {
            mapping: [
                (
                    PodIdentity::new("app", "instance", "master", "default", "1"),
                    NodeIdentity {
                        name: "NODE_1".to_string(),
                    },
                ),
                (
                    PodIdentity::new("app", "instance", "master", "default", "2"),
                    NodeIdentity {
                        name: "NODE_0".to_string(),
                    },
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        };

        let missing = [
            &PodIdentity::new("app", "instance", "worker", "default", "1"),
            &PodIdentity::new("app", "instance", "worker", "default", "2"),
            &PodIdentity::new("app", "instance", "history", "default", "1"),
        ];

        let preferred_nodes = [None, None, None];
        let strategy = GroupAntiAffinityStrategy::new(eligible_nodes, &current_mapping);
        let got = strategy.place(&missing, &preferred_nodes);

        assert_eq!(got.as_slice(), expected);
    }

    #[rstest]
    #[case::nothing_to_place(1, 1, 1, &[], &[])]
    #[case::not_enough_nodes(1, 0, 0, &[None], &[None])]
    #[case::place_one_pod(1, 0, 1, &[None], &[Some(NodeIdentity { name: "NODE_0".to_string() })])]
    #[case::place_one_pod_on_preferred(2, 0, 5, &[None, Some(NodeIdentity { name: "NODE_2".to_string() })], &[Some(NodeIdentity { name: "NODE_0".to_string() }), Some(NodeIdentity { name: "NODE_2".to_string() })])]
    #[case::place_three_pods(3, 0, 5, &[None, None, None],
        &[Some(NodeIdentity { name: "NODE_0".to_string() }),
          Some(NodeIdentity { name: "NODE_4".to_string() }),
          Some(NodeIdentity { name: "NODE_4".to_string() })])]
    #[case::place_three_pods_one_on_preferred(3, 0, 5, &[None, Some(NodeIdentity { name: "NODE_3".to_string() }), None],
        &[Some(NodeIdentity { name: "NODE_0".to_string() }),
          Some(NodeIdentity { name: "NODE_3".to_string() }),
          Some(NodeIdentity { name: "NODE_2".to_string() })])]
    fn test_scheduler_hashing_strategy(
        #[case] wanted_pod_count: usize,
        #[case] scheduled_pods_count: usize,
        #[case] available_node_count: usize,
        #[case] preferred_nodes: &[Option<NodeIdentity>],
        #[case] expected: &[Option<NodeIdentity>],
    ) {
        let wanted_pods = generate_ids(wanted_pod_count);
        let eligible_nodes = generate_eligible_nodes(available_node_count);

        let scheduled_pods: Vec<_> = wanted_pods
            .iter()
            .take(scheduled_pods_count)
            .cloned()
            .collect();
        let current_mapping = generate_current_mapping(&scheduled_pods, eligible_nodes.clone());

        let vec_preferred_nodes: Vec<Option<&NodeIdentity>> =
            preferred_nodes.iter().map(|o| o.as_ref()).collect();
        let strategy = HashingStrategy::new(&eligible_nodes);
        let got = strategy.place(
            current_mapping.missing(wanted_pods.as_slice()).as_slice(),
            vec_preferred_nodes.as_slice(),
        );
        assert_eq!(got, expected.to_vec());
    }

    #[rstest]
    #[case(1, None, "", "", None)]
    #[case(0, Some(NodeIdentity{name: "NODE_2".to_string()}), "ROLE_0", "GROUP_0", None)]
    #[case(3, Some(NodeIdentity{name: "NODE_0".to_string()}), "ROLE_0", "GROUP_1", Some(NodeIdentity{name: "NODE_2".to_string()}))] // node not found, use last!
    #[case(4, Some(NodeIdentity{name: "NODE_2".to_string()}), "ROLE_0", "GROUP_1", Some(NodeIdentity{name: "NODE_2".to_string()}))] // node found, use it!
    fn test_scheduler_preferred_node_or_last(
        #[case] eligible_node_count: usize,
        #[case] opt_node_id: Option<NodeIdentity>,
        #[case] role: &str,
        #[case] group: &str,
        #[case] expected: Option<NodeIdentity>,
    ) {
        let eligible_nodes = generate_eligible_nodes(eligible_node_count);
        let pod = PodIdentity {
            role: role.to_string(),
            group: group.to_string(),
            ..PodIdentity::default()
        };
        let got = eligible_nodes.preferred_node_or_last(&pod, opt_node_id.as_ref());

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

    #[rstest]
    #[case::no_missing_pods(1, 1, 1, vec![])]
    #[case::missing_one_pod(1, 0, 1, vec![PodIdentity { app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "1".to_string() }])]
    fn test_scheduler_pod_to_node_mapping_missing(
        #[case] wanted_pod_count: usize,
        #[case] scheduled_pods_count: usize,
        #[case] available_node_count: usize,
        #[case] expected: Vec<PodIdentity>,
    ) {
        let wanted_pods = generate_ids(wanted_pod_count);
        let available_nodes = generate_eligible_nodes(available_node_count);
        let scheduled_pods: Vec<_> = wanted_pods
            .iter()
            .take(scheduled_pods_count)
            .cloned()
            .collect();

        let mapping = generate_current_mapping(&scheduled_pods, available_nodes);

        let got = mapping.missing(wanted_pods.as_slice());
        let expected_refs: Vec<&PodIdentity> = expected.iter().collect();
        assert_eq!(got, expected_refs);
    }

    #[rstest]
    #[case::one_pod_is_scheduled(1, 1,
       Ok(SchedulerState {
           current_mapping: PodToNodeMapping::default(),
           remaining_mapping:
               PodToNodeMapping::new(vec![
                       (PodIdentity {app: "app".to_string(), instance: "simple".to_string(), role: "ROLE_0".to_string(), group: "GROUP_0".to_string(), id: "1".to_string() }, NodeIdentity { name: "NODE_0".to_string() }),
                   ])},
       ))]
    #[case::pod_cannot_be_scheduled(1, 0,
        Err(Error::NotEnoughNodesAvailable {
            number_of_nodes: 0,
            number_of_pods: 1,
            unscheduled_pods: vec![
                PodIdentity {
                    app: "app".to_string(),
                    instance: "simple".to_string(),
                    role: "ROLE_0".to_string(),
                    group: "GROUP_0".to_string(),
                    id: "1".to_string() }] }))]
    fn test_scheduler_update_history_and_result(
        #[case] pod_count: usize,
        #[case] node_count: usize,
        #[case] expected: SchedulerResult<SchedulerState>,
    ) {
        let pods = generate_ids(pod_count);
        let nodes = (0..pod_count)
            .map(|i| {
                if i < node_count {
                    Some(NodeIdentity {
                        name: format!("NODE_{}", i),
                    })
                } else {
                    None
                }
            })
            .collect();
        let current_mapping = PodToNodeMapping::default();
        let mut history = TestHistory::default();

        let mut scheduler = StickyScheduler::new(&mut history, ScheduleStrategy::GroupAntiAffinity);

        let got = scheduler.update_history_and_result(
            pods.iter().collect::<Vec<&PodIdentity>>(),
            nodes,
            pod_count,
            node_count,
            &current_mapping,
        );

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case(&[], "", Err(Error::PodIdentityFieldEmpty))]
    #[case(&[], "1", Err(Error::PodWithoutLabelsNotSupported))]
    #[case::no_app_label(&[(labels::APP_INSTANCE_LABEL, "myinstance"),
            (labels::APP_COMPONENT_LABEL, "myrole"),
            (labels::APP_ROLE_GROUP_LABEL, "mygroup")],
        "2",
        Err(Error::PodWithoutLabelsNotSupported))]
    #[case(&[(labels::APP_NAME_LABEL, "myapp"),
            (labels::APP_INSTANCE_LABEL, "myinstance"),
            (labels::APP_COMPONENT_LABEL, "myrole"),
            (labels::APP_ROLE_GROUP_LABEL, "mygroup")],
        "2",
        Ok(PodIdentity{
            app: "myapp".to_string(),
            instance: "myinstance".to_string(),
            role: "myrole".to_string(),
            group: "mygroup".to_string(),
            id: "2".to_string()}))]
    fn test_scheduler_pod_identity_try_from(
        #[case] labels: &[(&str, &str)],
        #[case] id: &str,
        #[case] expected: SchedulerResult<PodIdentity>,
    ) {
        let labels_map: BTreeMap<String, String> = labels
            .iter()
            .map(|t| (t.0.to_string(), t.1.to_string()))
            .collect();
        let pod = PodBuilder::new()
            .metadata(
                ObjectMetaBuilder::new()
                    .generate_name("pod1")
                    .namespace("default")
                    .with_labels(labels_map)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let got = PodIdentity::try_from_pod_and_id(&pod, id);

        assert_eq!(got, expected);
    }
}
