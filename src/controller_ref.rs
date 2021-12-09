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
        .as_ref()
        .and_then(|owners| {
            owners
                .iter()
                .find(|owner| matches!(owner.controller, Some(true)))
        })
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
