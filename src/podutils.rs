use k8s_openapi::api::core::v1::{Node, Pod, PodCondition, PodSpec, PodStatus};
use kube::api::Meta;
use std::fmt::{Debug, Display, Formatter, Result};

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

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Pod, PodCondition, PodSpec, PodStatus};
    use kube::api::ObjectMeta;

    #[test]
    fn test_is_pod_assigned_to_node() {
        let mut pod_spec = PodSpec::default();
        let mut pod = Pod {
            spec: Some(pod_spec.clone()),
            ..Pod::default()
        };

        assert!(!is_pod_assigned_to_node_name(&pod, "foobar"));

        pod_spec.node_name = Some("foobar".to_string());
        pod.spec = Some(pod_spec);
        assert!(is_pod_assigned_to_node_name(&pod, "foobar"));
        assert!(!is_pod_assigned_to_node_name(&pod, "barfoo"));

        pod.spec = None;
        assert!(!is_pod_assigned_to_node_name(&pod, "foobar"));
    }

    #[test]
    fn test_get_log_name() {
        let mut pod = Pod {
            metadata: ObjectMeta {
                name: Some("bar".to_string()),
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };

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
}
