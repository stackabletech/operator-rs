use crate::error::{Error, OperatorResult};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::Resource;
use kube::api::ObjectMeta;

pub fn object_to_owner_reference<K: Resource>(meta: ObjectMeta) -> OperatorResult<OwnerReference> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        name: meta.name.ok_or(Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.ok_or(Error::MissingObjectKey {
            key: ".metadata.backtrace",
        })?,
        ..OwnerReference::default()
    })
}
