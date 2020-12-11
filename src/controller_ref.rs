// This file is modeled after the K8S controller_ref.go file from the apimachinery package

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::Meta;

/// Returns a reference to the controller of the passed in resource if it exists.
pub fn get_controller_of<T>(resource: &T) -> Option<&OwnerReference>
where
    T: Meta,
{
    resource
        .meta()
        .owner_references
        .as_ref()
        .and_then(|owners| {
            owners
                .iter()
                .find(|owner| matches!(owner.controller, Some(true)))
        })
}

#[cfg(test)]
mod tests {
    use crate::controller_ref::get_controller_of;
    use k8s_openapi::api::core::v1::Pod;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;

    #[test]
    fn test_get_controller_of() {
        let mut pod = Pod::default();
        let controller = get_controller_of(&pod);
        assert_eq!(
            true,
            matches!(controller, None),
            "Did not expect an OwnerRefernce, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = Some(vec![OwnerReference {
            controller: Some(true),
            uid: "1234".to_string(),
            ..OwnerReference::default()
        }]);
        let controller = get_controller_of(&pod);
        assert_eq!(
            true,
            matches!(controller, Some(OwnerReference { uid, .. }) if uid == "1234"),
            "Expected a OwnerReference with uid 1234, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = Some(vec![OwnerReference {
            controller: None,
            uid: "1234".to_string(),
            ..OwnerReference::default()
        }]);
        let controller = get_controller_of(&pod);
        assert_eq!(
            true,
            matches!(controller, None),
            "Did not expect an OwnerReference, got [{:?}]",
            controller
        );

        pod.metadata.owner_references = Some(vec![
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
        ]);
        let controller = get_controller_of(&pod);
        assert_eq!(
            true,
            matches!(controller, Some(OwnerReference { uid, .. }) if uid == "5678"),
            "Expected a OwnerReference with uid 5678, got [{:?}]",
            controller
        );
    }
}
