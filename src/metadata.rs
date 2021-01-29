use crate::error::{Error, OperatorResult};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::Resource;
use kube::api::{Meta, ObjectMeta};
use std::collections::BTreeMap;

/// Builds a `ObjectMeta` object out of a few common options:
/// Automatically sets:
/// * name
/// * namespace (if the object passed in had one)
/// * labels (if provided)
/// * ownerReferences
pub fn build_metadata<T>(
    labels: Option<BTreeMap<String, String>>,
    resource: &T,
) -> OperatorResult<ObjectMeta>
where
    T: Meta,
{
    Ok(ObjectMeta {
        labels,
        name: Some(Meta::name(resource)),
        namespace: Meta::namespace(resource),
        owner_references: Some(vec![object_to_owner_reference::<T>(
            resource.meta().clone(),
        )?]),
        ..ObjectMeta::default()
    })
}

pub fn object_to_owner_reference<K: Resource>(meta: ObjectMeta) -> OperatorResult<OwnerReference> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        name: meta.name.ok_or(Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.ok_or(Error::MissingObjectKey {
            key: ".metadata.uid",
        })?,
        ..OwnerReference::default()
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    use k8s_openapi::api::core::v1::Pod;
    use rstest::rstest;

    #[rstest(name, namespace, case("foo", Some("bar")), case("foo", None))]
    fn test_build_metadata(name: &str, namespace: Option<&str>) -> OperatorResult<()> {
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());

        let name = Some(name.to_string());
        let namespace = namespace.map(|s| s.to_string());

        let pod = Pod {
            metadata: ObjectMeta {
                name: name.clone(),
                namespace: namespace.clone(),
                uid: Some("uid".to_string()),
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };

        let meta = build_metadata(Some(labels), &pod)?;

        assert_eq!(meta.name, name);
        assert_eq!(meta.namespace, namespace);

        let labels = meta.labels.unwrap();
        assert_eq!(labels.get("foo"), Some(&"bar".to_string()));
        assert_eq!(labels.len(), 1);

        Ok(())
    }
}
