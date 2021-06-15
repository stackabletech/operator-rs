use crate::error::{Error, OperatorResult};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::Resource;

use kube::api::{Resource, ResourceExt};
use std::collections::BTreeMap;

/// Builds a `ObjectMeta` object out of a template/owner object.
///
/// Automatically sets:
/// * name
/// * namespace (if the object passed in had one)
/// * labels (if provided)
/// * kubernetes recommended labels e.g. app.kubernetes.io/instance
/// * ownerReferences (pointing at the object that was passed in).
///
/// Caution:
/// The kubernetes recommended labels can be overwritten by
/// the labels provided by the user.
///
pub fn build_metadata<T>(
    name: String,
    labels: Option<BTreeMap<String, String>>,
    resource: &T,
    block_owner_deletion: bool,
) -> OperatorResult<ObjectMeta>
where
    T: Resource<DynamicType = ()>,
{
    let mut merged_labels = labels::get_recommended_labels(resource)?;

    if let Some(provided_labels) = labels {
        merged_labels.extend(provided_labels);
    }

    Ok(ObjectMeta {
        labels: Some(merged_labels),
        name: Some(name),
        namespace: resource.namespace(),
        owner_references: Some(vec![object_to_owner_reference::<T>(
            resource.meta(),
            block_owner_deletion,
        )?]),
        ..ObjectMeta::default()
    })
}

/// Creates an OwnerReference pointing to the resource type and `metadata` being passed in.
/// The created OwnerReference has it's `controller` flag set to `true`
pub fn object_to_owner_reference<K>(
    meta: &ObjectMeta,
    block_owner_deletion: bool,
) -> OperatorResult<OwnerReference>
where
    K: Resource<DynamicType = ()>,
{
    Ok(OwnerReference {
        api_version: K::api_version(&()).to_string(),
        kind: K::kind(&()).to_string(),
        name: meta.name.clone().ok_or(Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.clone().ok_or(Error::MissingObjectKey {
            key: ".metadata.uid",
        })?,
        controller: Some(true),
        block_owner_deletion: Some(block_owner_deletion),
    })
}
