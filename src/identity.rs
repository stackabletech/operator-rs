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
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

const POD_IDENTITY_FIELD_SEPARATOR: &str = ";";
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
    pub fn new(
        app: &str,
        instance: &str,
        role: &str,
        group: &str,
        id: &str,
    ) -> Result<Self, Error> {
        Self::assert_forbidden_char(app, instance, role, group, id)?;
        Ok(PodIdentity {
            app: app.to_string(),
            instance: instance.to_string(),
            role: role.to_string(),
            group: group.to_string(),
            id: id.to_string(),
        })
    }

    pub fn try_from_pod_and_id(pod: &Pod, id_label: &str) -> Result<Self, Error> {
        if id_label.is_empty() {
            return Err(Error::PodIdentityFieldEmpty);
        }

        match &pod.metadata.labels {
            Some(labels) => {
                let mut missing_labels = Vec::with_capacity(4);
                let mut app = String::new();
                let mut instance = String::new();
                let mut role = String::new();
                let mut group = String::new();
                let mut id = String::new();

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
                match labels.get(id_label).cloned() {
                    Some(value) => id = value,
                    _ => missing_labels.push(String::from(id_label)),
                }

                if missing_labels.is_empty() {
                    Ok(PodIdentity::new(
                        app.as_str(),
                        instance.as_str(),
                        role.as_str(),
                        group.as_str(),
                        id.as_str(),
                    )?)
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

    fn assert_forbidden_char(
        app: &str,
        instance: &str,
        role: &str,
        group: &str,
        id: &str,
    ) -> Result<(), Error> {
        let mut invalid_fields = BTreeMap::new();
        if app.contains(POD_IDENTITY_FIELD_SEPARATOR) {
            invalid_fields.insert(String::from("app"), String::from(app));
        }
        if instance.contains(POD_IDENTITY_FIELD_SEPARATOR) {
            invalid_fields.insert(String::from("instance"), String::from(instance));
        }
        if role.contains(POD_IDENTITY_FIELD_SEPARATOR) {
            invalid_fields.insert(String::from("role"), String::from(role));
        }
        if group.contains(POD_IDENTITY_FIELD_SEPARATOR) {
            invalid_fields.insert(String::from("group"), String::from(group));
        }
        if id.contains(POD_IDENTITY_FIELD_SEPARATOR) {
            invalid_fields.insert(String::from("id"), String::from(id));
        }

        if invalid_fields.is_empty() {
            Ok(())
        } else {
            Err(Error::PodIdentityFieldWithInvalidSeparator {
                separator: String::from(POD_IDENTITY_FIELD_SEPARATOR),
                invalid_fields,
            })
        }
    }
}
impl TryFrom<String> for PodIdentity {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Error> {
        let split = s.split(POD_IDENTITY_FIELD_SEPARATOR).collect::<Vec<&str>>();
        if split.len() != 5 {
            return Err(Error::PodIdentityNotParseable { pod_id: s });
        }
        PodIdentity::new(split[0], split[1], split[2], split[3], split[4])
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
        .join(POD_IDENTITY_FIELD_SEPARATOR)
    }
}

const DEFAULT_NODE_NAME: &str = "<no-nodename-set>";

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub name: String,
}

impl NodeIdentity {
    pub fn new(name: &str) -> Self {
        NodeIdentity {
            name: String::from(name),
        }
    }
}

impl TryFrom<&Pod> for NodeIdentity {
    type Error = Error;
    fn try_from(p: &Pod) -> Result<Self, Error> {
        let node = p
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .ok_or(Error::NodeWithoutNameNotSupported)?;

        Ok(NodeIdentity::new(node.as_ref()))
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
    /// Return a mapping for `pods` given that the `id_factory` can convert them to pod identities.
    /// Return an `Error` if any of the given pods cannot be converted to a pod identity or
    /// is not mapped to a node.
    /// # Argumens
    /// - `id_factory` : A factory that can build a `PodIdentity` from a `Pod`.
    /// - `pods` : The pods to add to the mapping.
    pub fn try_from(id_factory: &dyn PodIdentityFactory, pods: &[Pod]) -> Result<Self, Error> {
        let mut mapping = BTreeMap::default();

        for (pod_id, pod) in id_factory.try_map(pods)?.iter().zip(pods) {
            mapping.insert(pod_id.clone(), NodeIdentity::try_from(pod)?);
        }

        Ok(PodToNodeMapping { mapping })
    }

    pub fn get(&self, pod_id: &PodIdentity) -> Option<&NodeIdentity> {
        self.mapping.get(pod_id)
    }

    pub fn insert(&mut self, pod_id: PodIdentity, node_id: NodeIdentity) -> Option<NodeIdentity> {
        self.mapping.insert(pod_id, node_id)
    }

    /// Returns a map where entries are filtered by the given arguments.
    /// # Arguments
    /// - `app` : Application name.
    /// - `instance` : Application instance name.
    /// - `role` : Role name.
    /// - `group` : Group name.
    pub fn filter(
        &self,
        app: &str,
        instance: &str,
        role: &str,
        group: &str,
    ) -> BTreeMap<PodIdentity, NodeIdentity> {
        self.mapping
            .iter()
            .filter_map(|(pod_id, node_id)| {
                if pod_id.app() == app
                    && pod_id.instance() == instance
                    && pod_id.role() == role
                    && pod_id.group() == group
                {
                    Some((pod_id.clone(), node_id.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn merge(&self, other: &Self) -> Self {
        PodToNodeMapping {
            mapping: self
                .mapping
                .clone()
                .into_iter()
                .chain(other.mapping.clone().into_iter())
                .collect(),
        }
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
            result.insert(p, n);
        }
        PodToNodeMapping { mapping: result }
    }
}

/// A pod identity generator that can be implemented by the operators.
///
/// Implementation of this trait are responsible for:
/// - generating all pod identities expected by the service.
/// - map pods to their identities by implementing `try_map`
pub trait PodIdentityFactory: AsRef<[PodIdentity]> {
    /// Returns a PodToNodeMapping for the given pods or an error if any pod could not be mapped.
    fn try_map(&self, pods: &[Pod]) -> Result<Vec<PodIdentity>, Error>;
}

/// An implementation of [`PodIdentityFactory`] where id's are incremented across all roles and groups
/// contained in `eligible_nodes`.
///
/// This factory requires pods to be labeled with all `REQUIRED_LABELS` and with one additional "id label"
/// that can vary from operator to operator.
///
/// See `generate_ids` for details.
pub struct LabeledPodIdentityFactory {
    app: String,
    instance: String,
    id_label_name: String,
    slice: Vec<PodIdentity>,
}

impl LabeledPodIdentityFactory {
    /// Build a new instance of this factory.
    ///
    /// See `Self::generate_ids` for implemtation details.
    ///
    /// # Arguments
    /// - `app` : Application name.
    /// - `instance` : Application name.
    /// - `eligible_nodes` : Eligible nodes (and pod replicas) grouped by role and group.
    /// - `id_label_name` : Name of the pod's id label used to store the `id` field of `PodIdentity`
    /// - `start` : The initial value when generating the `id` fields of pod identities.
    pub fn new(
        app: &str,
        instance: &str,
        eligible_nodes: &EligibleNodesForRoleAndGroup,
        id_label_name: &str,
        start: usize,
    ) -> Self {
        LabeledPodIdentityFactory {
            app: app.to_string(),
            instance: instance.to_string(),
            id_label_name: id_label_name.to_string(),
            slice: Self::generate_ids(app, instance, eligible_nodes, start),
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
    fn generate_ids(
        app_name: &str,
        instance: &str,
        eligible_nodes: &EligibleNodesForRoleAndGroup,
        start: usize,
    ) -> Vec<PodIdentity> {
        let mut generated_ids = vec![];
        let mut id = start;
        // sorting role and group to keep the output consistent and make this
        // function testable.
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

        generated_ids
    }
}

impl AsRef<[PodIdentity]> for LabeledPodIdentityFactory {
    fn as_ref(&self) -> &[PodIdentity] {
        self.slice.as_ref()
    }
}

impl PodIdentityFactory for LabeledPodIdentityFactory {
    /// Returns a `PodToNodeMapping` for the given `pods`.
    /// Returns an `error::Error` if any of the pods doesn't have the expected labels
    /// or if any of the labels are invalid. A label is invalid if it doesn't match
    /// the corresponding field in `Self` like `app` or `instance`.
    /// # Arguments
    /// - `pods` : A pod slice.
    fn try_map(&self, pods: &[Pod]) -> Result<Vec<PodIdentity>, Error> {
        let mut result = vec![];

        for pod in pods {
            let pod_id = PodIdentity::try_from_pod_and_id(pod, self.id_label_name.as_ref())?;
            result.push(self.fields_match(pod_id)?);
        }
        Ok(result)
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::builder::{ObjectMetaBuilder, PodBuilder};
    use crate::role_utils::EligibleNodesAndReplicas;
    use rstest::*;
    use std::collections::{BTreeMap, HashMap};

    pub const APP_NAME: &str = "app";
    pub const INSTANCE: &str = "simple";

    #[rstest]
    #[case(&[], "", Err(Error::PodIdentityFieldEmpty))]
    #[case(&[], "ID_LABEL",
        Err(Error::PodWithoutLabelsNotSupported([
            labels::APP_NAME_LABEL,
            labels::APP_INSTANCE_LABEL,
            labels::APP_COMPONENT_LABEL,
            labels::APP_ROLE_GROUP_LABEL,
            "ID_LABEL"
        ].iter().map(|s| String::from(*s)).collect())))]
    #[case::no_app_and_id_label(&[
            (labels::APP_INSTANCE_LABEL, "myinstance"),
            (labels::APP_COMPONENT_LABEL, "myrole"),
            (labels::APP_ROLE_GROUP_LABEL, "mygroup")],
        "ID_LABEL",
        Err(Error::PodWithoutLabelsNotSupported([
            labels::APP_NAME_LABEL,
            "ID_LABEL"
        ].iter().map(|s| String::from(*s)).collect())))]
    #[case(&[(labels::APP_NAME_LABEL, "myapp"),
            (labels::APP_INSTANCE_LABEL, "myinstance"),
            (labels::APP_COMPONENT_LABEL, "myrole"),
            (labels::APP_ROLE_GROUP_LABEL, "mygroup"),
            ("ID_LABEL", "123")],
        "ID_LABEL",
        Ok(PodIdentity{
            app: "myapp".to_string(),
            instance: "myinstance".to_string(),
            role: "myrole".to_string(),
            group: "mygroup".to_string(),
            id: "123".to_string()}))]
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
    #[case::generate_one_id(0, vec![("role", "group", 0, 1)], vec![PodIdentity::new(APP_NAME, INSTANCE, "role", "group", "0").unwrap()])]
    #[case::generate_one_id_starting_at_1000(1000, vec![("role", "group", 0, 1)], vec![PodIdentity::new(APP_NAME, INSTANCE, "role", "group", "1000").unwrap()])]
    #[case::generate_five_ids(1,
        vec![
            ("master", "default", 0, 2),
            ("worker", "default", 0, 2),
            ("history", "default", 0, 1),
        ],
        vec![
            PodIdentity::new(APP_NAME, INSTANCE, "history", "default", "1").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "master", "default", "2").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "master", "default", "3").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "worker", "default", "4").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "worker", "default", "5").unwrap(),
        ]
    )]
    #[case::generate_two_roles(10,
        vec![
            ("role1", "default", 0, 2),
            ("role2", "default", 0, 1),
        ],
        vec![
            PodIdentity::new(APP_NAME, INSTANCE, "role1", "default", "10").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "role1", "default", "11").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "role2", "default", "12").unwrap(),
        ]
    )]
    fn test_identity_labeled_factory_as_slice(
        #[case] start: usize,
        #[case] nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
        #[case] expected: Vec<PodIdentity>,
    ) {
        let eligible_nodes_and_replicas = build_eligible_nodes_and_replicas(nodes_and_replicas);
        let factory = LabeledPodIdentityFactory::new(
            APP_NAME,
            INSTANCE,
            &eligible_nodes_and_replicas,
            "ID_LABEL",
            start,
        );
        let got = factory.as_ref();
        assert_eq!(got, expected.as_slice());
    }

    #[rstest]
    #[case(0, vec![], vec![], Ok(vec![]))]
    #[case(1000,
        vec![("role", "group", 1, 1)],
        vec![
            ("node_1", vec![
                (labels::APP_NAME_LABEL, APP_NAME),
                (labels::APP_INSTANCE_LABEL, INSTANCE),
                (labels::APP_COMPONENT_LABEL, "role"),
                (labels::APP_ROLE_GROUP_LABEL, "group"),
                ("ID_LABEL", "1000")]),],
        Ok(vec![
            PodIdentity::new(APP_NAME, INSTANCE, "role", "group", "1000").unwrap(),
        ]))]
    #[case(1000,
        vec![("master", "default", 1, 1)],
        vec![
            ("node_1", vec![
                (labels::APP_NAME_LABEL, "this-pod-belongs-to-another-app"),
                (labels::APP_INSTANCE_LABEL, INSTANCE),
                (labels::APP_COMPONENT_LABEL, "master"),
                (labels::APP_ROLE_GROUP_LABEL, "default"),
                ("ID_LABEL", "1000")]),],
        Err(Error::UnexpectedPodIdentityField {
            field: APP_NAME.to_string(),
            value: "this-pod-belongs-to-another-app".to_string(),
            expected: APP_NAME.to_string()
        }))]
    fn test_identity_labeled_factory_try_from(
        #[case] start: usize,
        #[case] nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
        #[case] pod_labels: Vec<(&str, Vec<(&str, &str)>)>,
        #[case] expected: Result<Vec<PodIdentity>, Error>,
    ) {
        let eligible_nodes_and_replicas = build_eligible_nodes_and_replicas(nodes_and_replicas);
        let pods = build_pods(pod_labels);
        let factory = LabeledPodIdentityFactory::new(
            APP_NAME,
            INSTANCE,
            &eligible_nodes_and_replicas,
            "ID_LABEL",
            start,
        );
        let got = factory.try_map(pods.as_slice());

        // Cannot compare `SchedulerResult`s directly because `crate::error::Error` doesn't implement `PartialEq`
        match (&got, &expected) {
            (Ok(g), Ok(e)) => assert_eq!(g, e),
            (Err(ge), Err(re)) => assert_eq!(format!("{:?}", ge), format!("{:?}", re)),
            _ => panic!("got: {:?}\nexpected: {:?}", got, expected),
        }
    }

    #[rstest]
    #[case(1, vec![], vec![], Ok(vec![]))]
    #[case(1,
        vec![("role", "group", 1, 1)],
        vec![
            ("node_1", vec![
                (labels::APP_NAME_LABEL, APP_NAME),
                (labels::APP_INSTANCE_LABEL, INSTANCE),
                (labels::APP_COMPONENT_LABEL, "role"),
                (labels::APP_ROLE_GROUP_LABEL, "group"),
                ("ID_LABEL", "1000")]),],
        Ok(vec![("node_1", PodIdentity::new(APP_NAME, INSTANCE, "role", "group", "1000").unwrap())])
    )]
    fn test_identity_pod_mapping_try_from(
        #[case] start: usize,
        #[case] nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
        #[case] pod_labels: Vec<(&str, Vec<(&str, &str)>)>,
        #[case] expected: Result<Vec<(&str, PodIdentity)>, Error>,
    ) {
        let pods = build_pods(pod_labels);
        let eligible_nodes_and_replicas = build_eligible_nodes_and_replicas(nodes_and_replicas);
        let factory = LabeledPodIdentityFactory::new(
            APP_NAME,
            INSTANCE,
            &eligible_nodes_and_replicas,
            "ID_LABEL",
            start,
        );

        let got = PodToNodeMapping::try_from(&factory, pods.as_slice());

        // Cannot compare `SchedulerResult`s directly because `crate::error::Error` doesn't implement `PartialEq`
        match (&got, &expected) {
            (Ok(g), Ok(e)) => assert_eq!(
                g,
                &PodToNodeMapping {
                    mapping: e
                        .iter()
                        .map(|(node, id)| (
                            id.clone(),
                            NodeIdentity {
                                name: node.to_string()
                            }
                        ))
                        .collect()
                }
            ),
            (Err(ge), Err(re)) => assert_eq!(format!("{:?}", ge), format!("{:?}", re)),
            _ => panic!("got: {:?}\nexpected: {:?}", got, expected),
        }
    }

    #[rstest]
    #[case(vec![], vec![], vec![])]
    #[case(
        vec![],
        vec![ ("node_1", APP_NAME, INSTANCE, "role1", "group1", "50")],
        vec![PodIdentity::new(APP_NAME, INSTANCE, "role1", "group1", "50").unwrap()])]
    #[case(
        vec![PodIdentity::new(APP_NAME, INSTANCE, "role1", "group1", "51").unwrap()],
        vec![ ("node_1", APP_NAME, INSTANCE, "role1", "group1", "50")],
        vec![
            PodIdentity::new(APP_NAME, INSTANCE, "role1", "group1", "50").unwrap(),
            PodIdentity::new(APP_NAME, INSTANCE, "role1", "group1", "51").unwrap(),
        ])]
    fn test_identity_pod_mapping_missing(
        #[case] expected: Vec<PodIdentity>,
        #[case] mapping_node_pod_id: Vec<(&str, &str, &str, &str, &str, &str)>,
        #[case] pod_ids: Vec<PodIdentity>,
    ) {
        let mapping = build_mapping(mapping_node_pod_id);
        let got = mapping.missing(pod_ids.as_slice());

        assert_eq!(got, expected);
    }

    pub fn build_pods(node_and_labels: Vec<(&str, Vec<(&str, &str)>)>) -> Vec<Pod> {
        let mut result = vec![];

        for (node_name, pod_labels) in node_and_labels {
            let labels_map: BTreeMap<String, String> = pod_labels
                .iter()
                .map(|t| (t.0.to_string(), t.1.to_string()))
                .collect();

            result.push(
                PodBuilder::new()
                    .metadata(
                        ObjectMetaBuilder::new()
                            .namespace("default")
                            .with_labels(labels_map)
                            .build()
                            .unwrap(),
                    )
                    .node_name(node_name)
                    .build()
                    .unwrap(),
            );
        }
        result
    }

    pub fn build_eligible_nodes_and_replicas(
        nodes_and_replicas: Vec<(&str, &str, usize, usize)>,
    ) -> EligibleNodesForRoleAndGroup {
        let mut eligible_nodes: HashMap<String, HashMap<String, EligibleNodesAndReplicas>> =
            HashMap::new();
        for (role, group, _node_count, replicas) in nodes_and_replicas {
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
                .or_insert_with(|| {
                    vec![(
                        group.to_string(),
                        EligibleNodesAndReplicas {
                            nodes: vec![],
                            replicas: Some(replicas as u16),
                        },
                    )]
                    .into_iter()
                    .collect()
                });
        }
        eligible_nodes
    }

    pub fn build_mapping(
        node_pod_id: Vec<(&str, &str, &str, &str, &str, &str)>,
    ) -> PodToNodeMapping {
        let mut mapping: BTreeMap<PodIdentity, NodeIdentity> = BTreeMap::default();
        for (node_name, app, instance, role, group, id) in node_pod_id {
            mapping.insert(
                PodIdentity::new(app, instance, role, group, id).unwrap(),
                NodeIdentity {
                    name: String::from(node_name),
                },
            );
        }
        PodToNodeMapping { mapping }
    }
}
