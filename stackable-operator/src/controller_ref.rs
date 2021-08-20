// This file is modeled after the K8S controller_ref.go file from the apimachinery package

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::Resource;

/// Returns a reference to the controller of the passed in resource if it exists.
pub fn get_controller_of<T>(resource: &T) -> Option<&OwnerReference>
where
    T: Resource,
{
    resource
        .meta()
        .owner_references
        .iter()
        .find(|owner| matches!(owner.controller, Some(true)))
}

/// This returns `false` for Resources that have no OwnerReference (with a Controller flag)
/// or where the Controller does not have the same `uid` as the passed in `owner_uid`.
/// If however the `uid` exists and matches we return `true`.
pub fn is_resource_owned_by<T>(resource: &T, owner_uid: &str) -> bool
where
    T: Resource,
{
    let controller = get_controller_of(resource);
    matches!(controller, Some(OwnerReference { uid, .. }) if uid == owner_uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::Pod;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use kube::api::ObjectMeta;

    #[test]
    fn test_get_controller_of() {
        let mut pod = Pod::default();
        let controller = get_controller_of(&pod);
        assert!(
            matches!(controller, None),
            "Did not expect an OwnerReference, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = vec![OwnerReference {
            controller: Some(true),
            uid: "1234".to_string(),
            ..OwnerReference::default()
        }];
        let controller = get_controller_of(&pod);
        assert!(
            matches!(controller, Some(OwnerReference { uid, .. }) if uid == "1234"),
            "Expected a OwnerReference with uid 1234, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = vec![OwnerReference {
            controller: None,
            uid: "1234".to_string(),
            ..OwnerReference::default()
        }];
        let controller = get_controller_of(&pod);
        assert!(
            matches!(controller, None),
            "Did not expect an OwnerReference, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = vec![
            OwnerReference {
                controller: None,
                uid: "1234".to_string(),
                ..OwnerReference::default()
            },
            OwnerReference {
                controller: Some(true),
                uid: "5678".to_string(),
                ..OwnerReference::default()
            },
        ];
        let controller = get_controller_of(&pod);
        assert!(
            matches!(controller, Some(OwnerReference { uid, .. }) if uid == "5678"),
            "Expected a OwnerReference with uid 5678, got [{:?}]",
            controller
        );
    }

    #[test]
    fn test_is_resource_owned_by() {
        let mut pod = Pod {
            metadata: ObjectMeta {
                name: Some("Foobar".to_string()),
                owner_references: vec![OwnerReference {
                    controller: Some(true),
                    uid: "1234-5678".to_string(),
                    ..OwnerReference::default()
                }],
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };

        assert!(is_resource_owned_by(&pod, "1234-5678"));

        pod.metadata.owner_references = vec![];
        assert!(!is_resource_owned_by(&pod, "1234-5678"));
    }
}
