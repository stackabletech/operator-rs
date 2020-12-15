use core::fmt;
use k8s_openapi::api::core::v1::{Pod, PodCondition, PodStatus};

/// While the `phase` field of a Pod is a string only the values from this enum are allowed.
#[derive(Debug, Eq, PartialEq)]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl fmt::Display for PodPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
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
    match &pod.status {
        Some(status) => {
            if let Some(phase) = &status.phase {
                if phase != "Running" {
                    // TODO: Replace with PodPhase comparison, I just don't know how
                    return false;
                }
            }

            is_pod_ready_condition_true(status)
        }

        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use crate::podutils::{get_pod_condition, is_pod_created, is_pod_running_and_ready};
    use k8s_openapi::api::core::v1::{Pod, PodCondition, PodStatus};

    #[test]
    fn test_is_pod_created() {
        assert_eq!(false, is_pod_created(None));

        let mut pod = Pod { ..Pod::default() };
        assert_eq!(false, is_pod_created(Some(&pod)));

        pod.status = Some(PodStatus {
            phase: Some("".to_string()),
            ..PodStatus::default()
        });
        assert_eq!(false, is_pod_created(Some(&pod)));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            ..PodStatus::default()
        });
        assert_eq!(true, is_pod_created(Some(&pod)));
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
        assert_eq!(false, is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            ..PodStatus::default()
        });
        assert_eq!(false, is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            ..PodStatus::default()
        });
        assert_eq!(false, is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert_eq!(false, is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                status: "False".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert_eq!(false, is_pod_running_and_ready(&pod));

        pod.status = Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                status: "True".to_string(),
                ..PodCondition::default()
            }]),
            ..PodStatus::default()
        });
        assert_eq!(true, is_pod_running_and_ready(&pod));
    }
}
