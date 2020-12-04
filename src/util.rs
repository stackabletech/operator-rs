use k8s_openapi::api::core::v1::{Pod, PodStatus};

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

#[cfg(test)]
mod tests {
    use crate::util::is_pod_created;
    use k8s_openapi::api::core::v1::{Pod, PodStatus};

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
}
