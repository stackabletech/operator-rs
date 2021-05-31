use crate::error::{Error, OperatorResult};

use crate::labels;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::Resource;
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
    Ok(ObjectMeta {
        labels,
        name: Some(name),
        namespace: Resource::namespace(resource),
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

#[cfg(test)]
mod tests {

    use super::*;

    use crate::labels::APP_INSTANCE_LABEL;
    use k8s_openapi::api::core::v1::Pod;
    use rstest::rstest;

    #[rstest]
    #[case("foo", Some("bar"))]
    #[case("foo", None)]
    fn test_build_metadata(
        #[case] name: &str,
        #[case] namespace: Option<&str>,
    ) -> OperatorResult<()> {
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());

        let namespace = namespace.map(|s| s.to_string());

        let pod = Pod {
            metadata: ObjectMeta {
                name: Some("foo_pod".to_string()),
                namespace: namespace.clone(),
                uid: Some("uid".to_string()),
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };

        let meta = build_metadata(name.to_string(), Some(labels), &pod, true)?;

        assert_eq!(meta.name, Some(name.to_string()));
        assert_eq!(meta.namespace, namespace);

        let labels = meta.labels.unwrap();
        assert_eq!(labels.get("foo"), Some(&"bar".to_string()));
        assert_eq!(labels.get(APP_INSTANCE_LABEL), Some(&"foo_pod".to_string()));
        assert_eq!(labels.len(), 2);

        Ok(())
    }
}
