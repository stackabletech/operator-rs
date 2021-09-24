///! This module implements structs and traits for pod and node identity management. They are the
///! building blocks of pod scheduling as implemented in the scheduler module.
///!
///! Operators are expected to implement the [`PodIdentityFactory`] trait or use the implementation
///! provided here called [`LabeledPodIdentityFactory`].
///!
///! Useful structs and their meaning:
///! * [`PodIdentity`] : identifies a pod from the set of all pods managed by an operator.
///! * [`NodeIdentity`] : identifies a node from the set of eligible nodes available to the operator.
///! * [`PodToNodeMapping`] : Describes the node where pods are assigned.
///
use crate::error::Error;
use crate::labels;
use crate::role_utils::{EligibleNodesAndReplicas, EligibleNodesForRoleAndGroup};
use k8s_openapi::api::core::v1::{Node, Pod};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::Iter;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use tracing::warn;

const SEMICOLON: &str = ";";
pub const REQUIRED_LABELS: [&str; 4] = [
    labels::APP_NAME_LABEL,
    labels::APP_INSTANCE_LABEL,
    labels::APP_COMPONENT_LABEL,
    labels::APP_ROLE_GROUP_LABEL,
];

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
    /// If any required pod labels are missing, returns a [`Error::PodWithoutLabelsNotSupported`].
    pub fn labels(pod: &Pod) -> Result<String, Error> {
        if pod.metadata.labels.is_none() {
            return Err(Error::PodWithoutLabelsNotSupported(
                REQUIRED_LABELS.iter().map(|s| String::from(*s)).collect(),
            ));
        }

        let mut result: Vec<String> = vec![];

        let pod_labels = &pod.metadata.labels.as_ref().unwrap();
        let mut missing_labels = Vec::with_capacity(REQUIRED_LABELS.len());
        for label_name in REQUIRED_LABELS {
            match pod_labels.get(label_name).cloned() {
                Some(value) => result.push(value),
                _ => missing_labels.push(label_name.to_string()),
            }
        }

        if missing_labels.is_empty() {
            Ok(result.join(","))
        } else {
            Err(Error::PodWithoutLabelsNotSupported(missing_labels))
        }
    }

    pub fn try_from_pod_and_id(pod: &Pod, id: &str) -> Result<Self, Error> {
        if id.is_empty() {
            return Err(Error::PodIdentityFieldEmpty);
        }

        match &pod.metadata.labels {
            Some(labels) => {
                let mut missing_labels = Vec::with_capacity(4);
                let mut app = String::new();
                let mut instance = String::new();
                let mut role = String::new();
                let mut group = String::new();

                match labels.get(labels::APP_NAME_LABEL).cloned() {
                    Some(value) => app = value,
                    _ => missing_labels.push(String::from(labels::APP_NAME_LABEL)),
                }
                match labels.get(labels::APP_INSTANCE_LABEL).cloned() {
                    Some(value) => instance = value,
                    _ => missing_labels.push(String::from(labels::APP_INSTANCE_LABEL)),
                }
                match labels.get(labels::APP_COMPONENT_LABEL).cloned() {
                    Some(value) => role = value,
                    _ => missing_labels.push(String::from(labels::APP_COMPONENT_LABEL)),
                }
                match labels.get(labels::APP_ROLE_GROUP_LABEL).cloned() {
                    Some(value) => group = value,
                    _ => missing_labels.push(String::from(labels::APP_ROLE_GROUP_LABEL)),
                }

                if missing_labels.is_empty() {
                    Ok(PodIdentity::new(
                        app.as_str(),
                        instance.as_str(),
                        role.as_str(),
                        group.as_str(),
                        id,
                    ))
                } else {
                    Err(Error::PodWithoutLabelsNotSupported(missing_labels))
                }
            }
            _ => Err(Error::PodWithoutLabelsNotSupported(
                REQUIRED_LABELS.iter().map(|s| String::from(*s)).collect(),
            )),
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

const DEFAULT_NODE_NAME: &str = "<no-nodename-set>";

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub name: String,
}

impl TryFrom<&Pod> for NodeIdentity {
    type Error = Error;
    fn try_from(p: &Pod) -> Result<Self, Error> {
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

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodToNodeMapping {
    pub mapping: BTreeMap<PodIdentity, NodeIdentity>,
}

impl PodToNodeMapping {
    pub fn iter(&self) -> Iter<'_, PodIdentity, NodeIdentity> {
        self.mapping.iter()
    }

    pub fn get_filtered(&self, role: &str, group: &str) -> BTreeMap<PodIdentity, NodeIdentity> {
        let mut filtered = BTreeMap::new();
        for (pod_id, node_id) in &self.mapping {
            if role == pod_id.role() && pod_id.group() == group {
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
                if pod_id.app() == id.app()
                    && pod_id.instance() == id.instance()
                    && pod_id.role() == id.role()
                    && pod_id.group() == id.group()
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

    /// Return true if the `node` is already mapped by a pod from `role` and `group`.
    pub fn mapped_by(&self, node: &NodeIdentity, role: &str, group: &str) -> bool {
        for (pod_id, mapped_node) in self.mapping.iter() {
            if node == mapped_node && pod_id.role() == role && pod_id.group() == group {
                return true;
            }
        }
        false
    }

    /// Given `pods` return all that are not mapped.
    pub fn missing(&self, pods: &[PodIdentity]) -> Vec<PodIdentity> {
        let mut result = vec![];
        for p in pods {
            if !self.mapping.contains_key(p) {
                result.push(p.clone())
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

/// Returns a Vec of pod identities according to the replica per role+group pair from `eligible_nodes`.
///
/// The `id` field is in the range from one (1) to the number of replicas per role+group. If no replicas
/// are defined, then the range goes from one (1) to the number of eligible groups.
///
/// Given a `start` value of 1000, a role with two groups where the first group has two replicas and
/// the second has three replicas, the generated `id` fields of the pod identities are counted as follows:
///
/// ```yaml
/// role_1:
///     - group_1:
///         - id: 1000
///         - id: 1001
///     - group_2:
///         - id: 1002
///         - id: 1003
///         - id: 1004
/// ```
///
/// # Arguments
/// * `app_name` - Application name
/// * `instance` - Service instance
/// * `eligible_nodes` - Eligible nodes grouped by role and groups.
/// * `start` - The starting value for the id field.
pub fn generate_ids(
    app_name: &str,
    instance: &str,
    eligible_nodes: &EligibleNodesForRoleAndGroup,
    start: usize,
) -> Vec<PodIdentity> {
    let mut generated_ids = vec![];
    let mut id = start;
    let sorted_nodes: BTreeMap<&String, &HashMap<String, EligibleNodesAndReplicas>> =
        eligible_nodes.iter().collect();
    for (role_name, groups) in sorted_nodes {
        let sorted_groups: BTreeMap<&String, &EligibleNodesAndReplicas> = groups
            .iter()
            .collect::<BTreeMap<&String, &EligibleNodesAndReplicas>>();
        for (group_name, eligible_nodes) in sorted_groups {
            let ids_per_group = eligible_nodes
                .replicas
                .map(usize::from)
                .unwrap_or_else(|| eligible_nodes.nodes.len());
            for _ in 0..ids_per_group {
                generated_ids.push(PodIdentity {
                    app: app_name.to_string(),
                    instance: instance.to_string(),
                    role: role_name.clone(),
                    group: group_name.clone(),
                    id: id.to_string(),
                });
                id += 1;
            }
        }
    }

    // Sort the result to make testing predictable. Otherwise the for-loop above is not
    // guaranteed to preserve insertion order so the tests might fail at random.
    //generated_ids.sort_by(|p| String::from(p.id()));
    generated_ids
}

/// A pod identity generator that can be implemented by the operators.
///
/// Implementation of this trait are responsible for:
/// - generating all pod identities expected by the service by implementing `PodIdentityFactory::as_slice`
/// - map pods to their identity values by implementing `PodIdentityFactory::try_mapping`
pub trait PodIdentityFactory {
    /// A slice with all pod identities that should exist for a given service.
    fn as_slice(&self) -> &[PodIdentity];
    /// Returns a PodToNodeMapping for the given pods or an error if any pod could not be mapped.
    fn try_mapping(&self, pods: &[Pod]) -> Result<PodToNodeMapping, Error>;
    /// A convenience implementation for quickly finding out which pods are not scheduled yet.
    fn missing(&self, pods: &[Pod]) -> Result<Vec<PodIdentity>, Error> {
        Ok(self.try_mapping(pods)?.missing(self.as_slice()))
    }
}

/// An implementation of [`PodIdentityFactory`] where id's are incremented across all roles and groups
/// contained in `eligible_nodes`.
///
/// This factory requires pods to be labeled with all `REQUIRED_LABELS` and with one additional "id label"
/// that can vary from operator to operator.
///
/// See `generate_ids` for details.
pub struct LabeledPodIdentityFactory<'a> {
    app: String,
    instance: String,
    eligible_nodes: &'a EligibleNodesForRoleAndGroup,
    id_label_name: String,
    slice: Vec<PodIdentity>,
}

impl<'a> LabeledPodIdentityFactory<'a> {
    pub fn new(
        app: &str,
        instance: &str,
        eligible_nodes: &'a EligibleNodesForRoleAndGroup,
        id_label_name: &str,
        start: usize,
    ) -> Self {
        LabeledPodIdentityFactory {
            app: app.to_string(),
            instance: instance.to_string(),
            eligible_nodes,
            id_label_name: id_label_name.to_string(),
            slice: generate_ids(app, instance, eligible_nodes, start),
        }
    }

    /// Returns the given `pod_id` if it's fields match those managed by `Self`
    /// This is a sanity check to make sure we don't mix pods that were not generated
    /// using this factory.
    fn fields_match(&self, pod_id: PodIdentity) -> Result<PodIdentity, Error> {
        if self.app != pod_id.app() {
            return Err(Error::UnexpectedPodIdentityField {
                field: "app".to_string(),
                value: pod_id.app().to_string(),
                expected: self.app.clone(),
            });
        }
        if self.instance != pod_id.instance() {
            return Err(Error::UnexpectedPodIdentityField {
                field: "instance".to_string(),
                value: pod_id.instance().to_string(),
                expected: self.instance.clone(),
            });
        }
        Ok(pod_id)
    }
}

impl PodIdentityFactory for LabeledPodIdentityFactory<'_> {
    /// Returns a `PodToNodeMapping` for the given `pods`.
    /// Returns an `error::Error` if any of the pods doesn't have the expected labels
    /// or if any of the labels are invalid. A label is invalid if it doesn't match
    /// the corresponding field in `Self` like `app` or `instance`.
    /// # Arguments
    /// - `pods` : A pod slice.
    fn try_mapping(&self, pods: &[Pod]) -> Result<PodToNodeMapping, Error> {
        let mut result = PodToNodeMapping::default();

        for pod in pods {
            match &pod.metadata.labels {
                Some(labels) => {
                    let id = labels.get(self.id_label_name.as_str()).ok_or_else(|| {
                        Error::PodWithoutLabelsNotSupported(vec![String::from(&self.id_label_name)])
                    })?;

                    let pod_id = PodIdentity::try_from_pod_and_id(pod, id.as_ref())?;
                    result.insert(self.fields_match(pod_id)?, NodeIdentity::try_from(pod)?);
                }
                None => {
                    return Err(Error::PodWithoutLabelsNotSupported(
                        REQUIRED_LABELS.iter().map(|s| String::from(*s)).collect(),
                    ))
                }
            }
        }
        Ok(result)
    }

    fn as_slice(&self) -> &[PodIdentity] {
        self.slice.as_slice()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::builder::{ObjectMetaBuilder, PodBuilder};
    use crate::role_utils::EligibleNodesAndReplicas;
    use rstest::*;
    use std::collections::{BTreeMap, HashMap};

    #[rstest]
    #[case(&[], "", Err(Error::PodIdentityFieldEmpty))]
    #[case(&[], "1", Err(Error::PodWithoutLabelsNotSupported(REQUIRED_LABELS.iter().map(|s| String::from(*s)).collect())))]
    #[case::no_app_label(&[
            (labels::APP_INSTANCE_LABEL, "myinstance"),
            (labels::APP_COMPONENT_LABEL, "myrole"),
            (labels::APP_ROLE_GROUP_LABEL, "mygroup")],
        "2",
        Err(Error::PodWithoutLabelsNotSupported([labels::APP_NAME_LABEL.to_string()].to_vec())))]
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
    fn test_identity_pod_identity_try_from_pod_and_id(
        #[case] labels: &[(&str, &str)],
        #[case] id: &str,
        #[case] expected: Result<PodIdentity, Error>,
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

        // Cannot compare `SchedulerResult`s directly because `crate::error::Error` doesn't implement `PartialEq`
        match (&got, &expected) {
            (Ok(g), Ok(e)) => assert_eq!(g, e),
            (Err(ge), Err(re)) => assert_eq!(format!("{:?}", ge), format!("{:?}", re)),
            _ => panic!("got: {:?}\nexpected: {:?}", got, expected),
        }
    }

    #[rstest]
    #[case(0, vec![], vec![])]
    #[case::generate_one_id(0, vec![("role", "group", 0, 1)], vec![PodIdentity::new("app", "instance", "role", "group", "0")])]
    #[case::generate_one_id_starting_at_1000(1000, vec![("role", "group", 0, 1)], vec![PodIdentity::new("app", "instance", "role", "group", "1000")])]
    #[case::generate_five_ids(1,
        vec![
            ("master", "default", 0, 2),
            ("worker", "default", 0, 2),
            ("history", "default", 0, 1),
        ],
        vec![
            PodIdentity::new("app", "instance", "history", "default", "1"),
            PodIdentity::new("app", "instance", "master", "default", "2"),
            PodIdentity::new("app", "instance", "master", "default", "3"),
            PodIdentity::new("app", "instance", "worker", "default", "4"),
            PodIdentity::new("app", "instance", "worker", "default", "5"),
        ]
    )]
    #[case::generate_two_roles(10,
        vec![
            ("role1", "default", 0, 2),
            ("role2", "default", 0, 1),
        ],
        vec![
            PodIdentity::new("app", "instance", "role1", "default", "10"),
            PodIdentity::new("app", "instance", "role1", "default", "11"),
            PodIdentity::new("app", "instance", "role2", "default", "12"),
        ]
    )]
    fn test_identity_labeled_factory_as_slice(
        #[case] start: usize,
        #[case] nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
        #[case] expected: Vec<PodIdentity>,
    ) {
        let eligible_nodes_and_replicas = build_eligible_nodes_and_replicas(nodes_and_replicas);
        let factory = LabeledPodIdentityFactory::new(
            "app",
            "instance",
            &eligible_nodes_and_replicas,
            "ID_LABEL",
            start,
        );
        let got = factory.as_slice();
        assert_eq!(got, expected.as_slice());
    }

    #[rstest]
    #[case(0, vec![], vec![], Ok(PodToNodeMapping::default()))]
    fn test_identity_labeled_factory_try_mapping(
        #[case] start: usize,
        #[case] nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
        #[case] pods: Vec<Pod>,
        #[case] expected: Result<PodToNodeMapping, Error>,
    ) {
        let eligible_nodes_and_replicas = build_eligible_nodes_and_replicas(nodes_and_replicas);
        let factory = LabeledPodIdentityFactory::new(
            "app",
            "instance",
            &eligible_nodes_and_replicas,
            "ID_LABEL",
            start,
        );
        let got = factory.try_mapping(pods.as_slice());

        // Cannot compare `SchedulerResult`s directly because `crate::error::Error` doesn't implement `PartialEq`
        match (&got, &expected) {
            (Ok(g), Ok(e)) => assert_eq!(g, e),
            (Err(ge), Err(re)) => assert_eq!(format!("{:?}", ge), format!("{:?}", re)),
            _ => panic!("got: {:?}\nexpected: {:?}", got, expected),
        }
    }

    fn build_eligible_nodes_and_replicas(
        nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
    ) -> EligibleNodesForRoleAndGroup {
        let mut eligible_nodes: HashMap<String, HashMap<String, EligibleNodesAndReplicas>> =
            HashMap::new();
        for (role, group, node_count, replicas) in nodes_and_replicas {
            eligible_nodes
                .entry(String::from(role))
                .and_modify(|r| {
                    r.insert(
                        String::from(group),
                        EligibleNodesAndReplicas {
                            nodes: vec![],
                            replicas: Some(replicas as u16),
                        },
                    );
                })
                .or_insert(
                    vec![(
                        group.to_string(),
                        EligibleNodesAndReplicas {
                            nodes: vec![],
                            replicas: Some(replicas as u16),
                        },
                    )]
                    .into_iter()
                    .collect(),
                );
        }
        eligible_nodes
    }
}
