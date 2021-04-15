use crate::error::OperatorResult;
use crate::metadata;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::Meta;
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
    T: Meta<DynamicType = ()>,
{
    let cm = ConfigMap {
        data: Some(data),
        metadata: ObjectMeta {
            name: Some(String::from(cm_name)),
            namespace: Meta::namespace(resource),
            owner_references: Some(vec![metadata::object_to_owner_reference::<T>(
                resource.meta(),
                true,
            )?]),
            ..ObjectMeta::default()
        },
        ..ConfigMap::default()
    };
    Ok(cm)
}
