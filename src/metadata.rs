use crate::error::{Error, OperatorResult};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::Resource;

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
