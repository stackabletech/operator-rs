use crate::error::OperatorResult;
use crate::metadata;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Resource, ResourceExt};
use std::collections::BTreeMap;

/// Creates a ConfigMap.
/// This ConfigMap has its `block_owner_deletion` flag set to true.
/// That means it'll be deleted if its owner is being deleted.
pub fn create_config_map<T>(
    resource: &T,
    cm_name: &str,
    data: BTreeMap<String, String>,
) -> OperatorResult<ConfigMap>
where
    T: Resource<DynamicType = ()>,
{
    let cm = ConfigMap {
        data,
        metadata: ObjectMeta {
            name: Some(String::from(cm_name)),
            namespace: resource.namespace(),
            owner_references: vec![metadata::object_to_owner_reference::<T>(
                resource.meta(),
                true,
            )?],
            ..ObjectMeta::default()
        },
        ..ConfigMap::default()
    };
    Ok(cm)
}
