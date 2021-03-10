use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter, Result};

use k8s_openapi::api::core::v1::{Node, Pod, PodCondition, PodSpec, PodStatus};
use kube::api::Meta;
use tracing::debug;

/// While the `phase` field of a Pod is a string only the values from this enum are allowed.
#[derive(Debug, Eq, PartialEq)]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl Display for PodPhase {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}

pub enum PodConditionType {
    ContainersReady,
    Initialized,
    Ready,
    PodScheduled,
}

/// Returns whether the Pod has been created in the API server by
/// checking whether the `status.phase` field exists and is not empty.
pub fn is_pod_created(pod: Option<&Pod>) -> bool {
    match pod {
        None
        | Some(Pod { status: None, .. })
        | Some(Pod {
            status: Some(PodStatus { phase: None, .. }),
            ..
        }) => false,
        Some(Pod {
            status:
                Some(PodStatus {
                    phase: Some(status),
                    ..
                }),
            ..
        }) if status.is_empty() => false,
        Some(_) => true,
    }
}

/// Reports whether a pod is running and ready by checking the phase of the pod as well as conditions.
/// The phase has to be "Running" and the "Ready" condition has to be `true`.
pub fn is_pod_running_and_ready(pod: &Pod) -> bool {
    let status = match &pod.status {
        Some(PodStatus {
            phase: Some(phase), ..
        }) if phase != "Running" => return false,
        Some(status) => status,
        _ => return false,
    };

    is_pod_ready_condition_true(status)
}

fn is_pod_ready_condition_true(status: &PodStatus) -> bool {
    match get_pod_condition(status, "Ready") {
        None => false,
        Some(PodCondition { status, .. }) => status == "True",
    }
}

// TODO: condition should be the enum PodConditionType
fn get_pod_condition<'a>(status: &'a PodStatus, condition: &str) -> Option<&'a PodCondition> {
    match &status.conditions {
        None => None,
        Some(conditions) => conditions.iter().find(|c| c.type_ == condition),
    }
}

/// Returns a name that is suitable for directly passing to a log macro.
///
/// It'll contain the namespace and the name wrapped in square brackets.
/// Example output: `[foo/bar]`
///
/// If the resource has no namespace, it'll print `<no namespace>` instead: `[<no namespace>/bar]`
pub fn get_log_name<T>(resource: &T) -> String
where
    T: Meta,
{
    format!(
        "[{}/{}]",
        Meta::namespace(resource).unwrap_or_else(|| "<no namespace>".to_string()),
        Meta::name(resource)
    )
}

/// Checks whether the given Pod is assigned to (via the `spec.node_name` field) the given `node_name`.
pub fn is_pod_assigned_to_node_name(pod: &Pod, node_name: &str) -> bool {
    matches!(pod.spec, Some(PodSpec { node_name: Some(ref pod_node_name), ..}, ..) if pod_node_name == node_name)
}

/// Checks whether the given Pod is assigned to (via the `spec.node_name` field) the given Node (via `metadata.name`).
pub fn is_pod_assigned_to_node(pod: &Pod, node: &Node) -> bool {
    matches!((pod.spec.as_ref(), node.metadata.name.as_ref()),
        (
            Some(PodSpec { node_name: Some(ref pod_node_name), ..}, ..),
            Some(node_node_name),
        ) if pod_node_name == node_node_name
    )
}

/// This method checks if a Pod contains all required labels including an optional check for values.
///
/// # Arguments
///
/// * `pod` - the Pod to check for labels
/// * `required_labels` - is a BTreeMap of label keys to an optional vector of label values.
///                       Multiple values can be passed in but the Pod must obviously match
///                       _any_ of the values to be accepted
///
/// # Example
///
/// ```
/// use stackable_operator::podutils;
/// # use k8s_openapi::api::core::v1::{Pod, PodSpec};
/// # use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
/// use std::collections::BTreeMap;
///
/// # let pod = Pod {
/// #            metadata: ObjectMeta {
/// #                ..ObjectMeta::default()
/// #            },
/// #            spec: None,
/// #            status: None,
/// #        };
///
/// let mut required_labels = BTreeMap::new();
/// required_labels.insert("foo".to_string(), None);
///
/// assert!(!podutils::pod_matches_multiple_label_values(&pod, &required_labels));
/// ```
pub fn pod_matches_multiple_label_values(
    pod: &Pod,
    required_labels: &BTreeMap<String, Option<Vec<String>>>,
) -> bool {
    let pod_labels = &pod.metadata.labels;

    for (expected_key, expected_value) in required_labels {
        // We only do this here because `expected_labels` might be empty in which case
        // it's totally fine if the Pod doesn't have any labels.
        // Now however we're definitely looking for a key so if the Pod doesn't have any labels
        // it will never be able to match.
        let pod_labels = match pod_labels {
            None => return false,
            Some(pod_labels) => pod_labels,
        };

        // We can match two kinds:
        //   * Just the label key (expected_value == None)
        //   * Key and Value
        if !pod_labels.contains_key(expected_key) {
            debug!(
                "Pod [{}] is missing label [{}]",
                Meta::name(pod),
                expected_key
            );
            return false;
        }

        if let Some(expected_values) = expected_value {
            // unwrap is fine here as we already checked earlier if the key exists
            let pod_value = pod_labels.get(expected_key).unwrap();

            if !expected_values.iter().any(|value| value == pod_value) {
                debug!("Pod [{}] has correct label [{}] but the wrong value (has: [{}], should have one of: [{:?}]", Meta::name(pod), expected_key, pod_value, expected_values);
                return false;
            }
        }
    }
    true
}

/// This method can be used to find Pods that are invalid.
///
/// It returns all Pods that return `false` when passed to the [`is_valid_pod`] method.
pub fn find_invalid_pods<'a>(
    pods: &'a [Pod],
    required_labels: &BTreeMap<String, Option<Vec<String>>>,
) -> Vec<&'a Pod> {
    pods.iter()
        .filter(|pod| !is_valid_pod(pod, required_labels))
        .collect()
}

/// Checks whether a Pod is valid or not.
///
/// For a Pod to be valid it must be assigned to a node (via `spec.node_name`) and it must
/// have all required labels.
/// See [`pod_matches_multiple_label_values`] for a description of the label format.
pub fn is_valid_pod(pod: &Pod, required_labels: &BTreeMap<String, Option<Vec<String>>>) -> bool {
    matches!(
        pod.spec,
        Some(PodSpec {
            node_name: Some(_),
            ..
        })
    ) && pod_matches_multiple_label_values(pod, required_labels)
}

/// This method can be used to find Pods that are not needed anymore.
///
/// For this to work we'll compare a list of all Pods against a list of Pods that are actively being used.
/// We'll do this for an arbitrary number of Node lists and match labels.
// TODO: Test and docs
pub fn find_excess_pods<'a>(
    nodes_and_labels: &[(Vec<Node>, BTreeMap<String, Option<String>>)],
    existing_pods: &'a [Pod],
) -> Vec<&'a Pod> {
    let mut used_pods = Vec::new();

    // For each pair of Nodes and labels we try to find all Pods that are currently in use and valid
    // We collect all of those in one big list.
    for (eligible_nodes, mandatory_label_values) in nodes_and_labels {
        let mut found_pods =
            find_pods_that_are_in_use(&eligible_nodes, &existing_pods, mandatory_label_values);
        used_pods.append(&mut found_pods);
    }

    // Here we'll filter all existing Pods and will remove all Pods that are in use
    existing_pods.iter()
        .filter(|pod| {
            !used_pods
                .iter()
                .any(|used_pod|
                    matches!((pod.metadata.uid.as_ref(), used_pod.metadata.uid.as_ref()), (Some(existing_uid), Some(used_uid)) if existing_uid == used_uid))
        })
        .collect()
}

/// This function can be used to get a list of Pods that are assigned (via their `spec.node_name` property)
/// to specific nodes.
///
/// This is useful to find all _valid_ pods (i.e. ones that are actually required by an Operator)
/// so it can be compared against _all_ Pods that belong to the Controller.
/// All Pods that are not actually in use can be deleted.
/// TODO: Docs
pub fn find_pods_that_are_in_use<'a>(
    candidate_nodes: &[Node],
    existing_pods: &'a [Pod],
    label_values: &BTreeMap<String, Option<String>>,
) -> Vec<&'a Pod> {
    existing_pods
        .iter()
        .filter(|pod|
            // This checks whether the Pod has all the required labels and if it does
            // it'll try to find a Node with the same `node_name` as the Pod.
            pod_matches_labels(pod, &label_values) && candidate_nodes.iter().any(|node| is_pod_assigned_to_node(pod, node))
        )
        .collect()
}

// TODO: Move to podutils and rename? matches_labels?
pub fn pod_matches_labels(pod: &Pod, expected_labels: &BTreeMap<String, Option<String>>) -> bool {
    let converted = expected_labels
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.as_ref().map(|string| vec![string.clone()]),
            )
        })
        .collect::<BTreeMap<_, _>>();
    pod_matches_multiple_label_values(pod, &converted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::{NodeBuilder, PodBuilder};
    use k8s_openapi::api::core::v1::{Pod, PodCondition, PodStatus};

    #[test]
    fn test_is_pod_assigned_to_node_name() {
        // Pod with no node_name
        let pod = PodBuilder::new().build();
        assert!(!is_pod_assigned_to_node_name(&pod, "foobar"));

        // Pod with node_name, matches one but not the other
        let mut pod = PodBuilder::new().node_name("foobar").build();
        assert!(is_pod_assigned_to_node_name(&pod, "foobar"));
        assert!(!is_pod_assigned_to_node_name(&pod, "barfoo"));

        // Pod with empty spec doesn't match
        pod.spec = None;
        assert!(!is_pod_assigned_to_node_name(&pod, "foobar"));
    }

    #[test]
    fn test_is_pod_assigned_to_node() {
        // Pod with no node_name
        let pod = PodBuilder::new().build();
        let node = NodeBuilder::new().name("foobar").build();
        let node2 = NodeBuilder::new().name("barfoo").build();
        assert!(!is_pod_assigned_to_node(&pod, &node));

        // Pod with node_name, matches one but not the other
        let mut pod = PodBuilder::new().node_name("foobar").build();
        assert!(is_pod_assigned_to_node(&pod, &node));
        assert!(!is_pod_assigned_to_node(&pod, &node2));

        // Pod with empty spec doesn't match
        pod.spec = None;
        assert!(!is_pod_assigned_to_node(&pod, &node));
    }

    #[test]
    fn test_get_log_name() {
        let mut pod = PodBuilder::new().name("bar").build();
        assert_eq!("[<no namespace>/bar]", get_log_name(&pod));

        pod.metadata.namespace = Some("foo".to_string());
        assert_eq!("[foo/bar]", get_log_name(&pod));
    }

    #[test]
    fn test_is_pod_created() {
        assert!(!is_pod_created(None));

        let mut pod = Pod { ..Pod::default() };
        assert!(!is_pod_created(Some(&pod)));

        pod.status = Some(PodStatus {
            phase: Some("".to_string()),
            ..PodStatus::default()
        });
        assert!(!is_pod_created(Some(&pod)));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            ..PodStatus::default()
        });
        assert!(is_pod_created(Some(&pod)));
    }

    #[test]
    fn test_get_pod_condition() {
        let status = PodStatus {
            conditions: Some(vec![]),
            ..PodStatus::default()
        };
        assert_eq!(None, get_pod_condition(&status, "doesntexist"));

        let condition = PodCondition {
            status: "OrNot".to_string(),
            type_: "Ready".to_string(),
            ..PodCondition::default()
        };
        let status = PodStatus {
            conditions: Some(vec![condition.clone()]),
            ..PodStatus::default()
        };
        assert_eq!(Some(&condition), get_pod_condition(&status, "Ready"));
    }

    #[test]
    fn test_pod_ready_and_running() {
        let mut pod = Pod { ..Pod::default() };
        assert!(!is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            ..PodStatus::default()
        });
        assert!(!is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            ..PodStatus::default()
        });
        assert!(!is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert!(!is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                status: "False".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert!(!is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                status: "True".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert!(is_pod_running_and_ready(&pod));
    }

    #[test]
    fn test_pod_matches_labels() {
        let mut test_labels = BTreeMap::new();
        test_labels.insert("label1".to_string(), "test1".to_string());
        test_labels.insert("label2".to_string(), "test2".to_string());
        test_labels.insert("label3".to_string(), "test3".to_string());

        let test_pod = PodBuilder::new().with_labels(test_labels).build();

        // Pod matches a label, should match
        let mut matching_labels1 = BTreeMap::new();
        matching_labels1.insert(String::from("label1"), Some(String::from("test1")));
        assert!(pod_matches_labels(&test_pod, &matching_labels1));

        // Pod matches a label, should match
        let mut matching_labels2 = BTreeMap::new();
        matching_labels2.insert(String::from("label2"), Some(String::from("test2")));
        assert!(pod_matches_labels(&test_pod, &matching_labels2));

        // Pods that are missing a label should not match
        let mut non_matching_labels1 = BTreeMap::new();
        non_matching_labels1.insert(String::from("wrong_label"), Some(String::from("test2")));
        assert!(!pod_matches_labels(&test_pod, &non_matching_labels1));

        // Empty list should match all pods - we have no requirements that the pod
        // has to meet
        let empty_labels = BTreeMap::new();
        assert!(pod_matches_labels(&test_pod, &empty_labels));

        // Pod matches only one of two required labels, should not match
        let mut non_matching_multiple_labels = BTreeMap::new();
        non_matching_multiple_labels.insert(String::from("label1"), Some(String::from("test1")));
        non_matching_multiple_labels.insert(String::from("label2"), Some(String::from("test1")));
        assert!(!pod_matches_labels(
            &test_pod,
            &non_matching_multiple_labels
        ));

        // Pod matches both labels, should match
        let mut matching_multiple_labels = BTreeMap::new();
        matching_multiple_labels.insert(String::from("label1"), Some(String::from("test1")));
        matching_multiple_labels.insert(String::from("label2"), Some(String::from("test2")));
        assert!(pod_matches_labels(&test_pod, &matching_multiple_labels));

        // Pod has required label without specified value, should match
        let mut matching_label_present = BTreeMap::new();
        matching_label_present.insert(String::from("label1"), None);
        assert!(pod_matches_labels(&test_pod, &matching_label_present));

        // Pod has multiple required labels without specified value, should match
        let mut matching_multiple_label_present = BTreeMap::new();
        matching_multiple_label_present.insert(String::from("label1"), None);
        matching_multiple_label_present.insert(String::from("label3"), None);
        assert!(pod_matches_labels(
            &test_pod,
            &matching_multiple_label_present
        ));

        // Pod has one label missing and one present - should not match
        let mut matching_label_present_and_missing = BTreeMap::new();
        matching_label_present_and_missing.insert(String::from("label1"), None);
        matching_label_present_and_missing.insert(String::from("label4"), None);
        assert!(!pod_matches_labels(
            &test_pod,
            &matching_label_present_and_missing
        ));

        // Pod has _no_ labels, should not match because we are definitely asking for labels
        let test_pod = PodBuilder::new().build();
        let mut matching_label_present_and_missing = BTreeMap::new();
        matching_label_present_and_missing.insert(String::from("label1"), None);
        matching_label_present_and_missing.insert(String::from("label4"), None);
        assert!(!pod_matches_labels(
            &test_pod,
            &matching_label_present_and_missing
        ));

        // Pod has _no_ labels but we're also asking for no labels
        assert!(pod_matches_labels(&test_pod, &BTreeMap::new()));
    }

    #[test]
    fn test_find_pods_that_are_in_use() {
        // Two nodes, one pod, no labels on pod, but looking for labels, shouldn't match
        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new().node_name("foobar").build()];

        let mut label_values = BTreeMap::new();
        label_values.insert("foo".to_string(), Some("bar".to_string()));

        assert_eq!(
            0,
            find_pods_that_are_in_use(&nodes, &existing_pods, &label_values).len()
        );

        // Two nodes, one pod, matching labels on pod, but looking for labels, should match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new()
            .node_name("foobar")
            .with_labels(pod_labels)
            .build()];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, one pod, matching label key on pod but wrong value, but looking for labels, shouldn't match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "WRONG".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new()
            .node_name("foobar")
            .with_labels(pod_labels)
            .build()];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            0,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, two pods. one matches the other doesn't
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![
            PodBuilder::new()
                .node_name("foobar")
                .with_labels(pod_labels.clone())
                .build(),
            PodBuilder::new()
                .node_name("wrong_node")
                .with_labels(pod_labels)
                .build(),
        ];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );
    }

    #[test]
    fn test_pod_matches_multiple_label_values() {
        let pod = PodBuilder::new().build();

        let mut required_labels = BTreeMap::new();

        // Pod has no labels but we don't require any either
        assert!(pod_matches_multiple_label_values(&pod, &required_labels));

        // Pod has no labels but we want one but don't care about the value
        required_labels.insert("foo".to_string(), None);
        assert!(!pod_matches_multiple_label_values(&pod, &required_labels));

        // Pod has only the required label
        let pod = PodBuilder::new().with_label("foo", "bar").build();
        assert!(pod_matches_multiple_label_values(&pod, &required_labels));

        // Pod has multiple labels
        let pod = PodBuilder::new()
            .with_label("foo", "bar")
            .with_label("bar", "foo")
            .build();
        assert!(pod_matches_multiple_label_values(&pod, &required_labels));

        // Pod has correct label but wrong value
        required_labels.insert("bar".to_string(), Some(vec!["baz".to_string()]));
        assert!(!pod_matches_multiple_label_values(&pod, &required_labels));

        // Pod cas correct label and also one of the correct values
        required_labels.insert(
            "bar".to_string(),
            Some(vec!["baz".to_string(), "foo".to_string()]),
        );
        assert!(pod_matches_multiple_label_values(&pod, &required_labels));
    }

    #[test]
    // We'll only test very basic things with the labels because it should all be covered by other tests already
    fn test_is_valid_pod() {
        let pod = PodBuilder::new().build();
        let mut required_labels = BTreeMap::new();

        // Pod is not assigned to a node
        assert!(!is_valid_pod(&pod, &required_labels));

        // Pod is assigned to a node and no labels required
        let pod = PodBuilder::new().node_name("foo").build();
        assert!(is_valid_pod(&pod, &required_labels));

        // Pod is missing label
        required_labels.insert("foo".to_string(), None);
        assert!(!is_valid_pod(&pod, &required_labels));

        // Pod has required label
        let pod = PodBuilder::new()
            .node_name("foo")
            .with_label("foo", "bar")
            .build();
        assert!(is_valid_pod(&pod, &required_labels));
    }

    #[test]
    // Most things will be covered by other tests so this one is very basic
    fn test_find_invalid_pods() {
        let valid_pod = PodBuilder::new().node_name("foo").build();
        let invalid_pod = PodBuilder::new().name("invalid").build();

        let required_labels = BTreeMap::new();

        let pods = vec![valid_pod.clone(), invalid_pod.clone()];
        let mut invalid_pods = find_invalid_pods(&pods, &required_labels);
        assert_eq!(invalid_pods.len(), 1);
        let invalid_pod = invalid_pods.remove(0);
        assert_eq!(Meta::name(invalid_pod), "invalid");

        let pods = vec![valid_pod.clone(), valid_pod.clone()];
        let invalid_pods = find_invalid_pods(&pods, &required_labels);
        assert!(invalid_pods.is_empty());

        let pods = vec![valid_pod.clone(), invalid_pod.clone(), invalid_pod.clone()];
        let invalid_pods = find_invalid_pods(&pods, &required_labels);
        assert_eq!(invalid_pods.len(), 2);
    }
}
