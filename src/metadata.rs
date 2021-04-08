use crate::error::{Error, OperatorResult};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::Resource;
use kube::api::{Meta, ObjectMeta};
use std::collections::BTreeMap;

/// The name of the application	e.g. "mysql"
pub const APP_KUBERNETES_IO_NAME: &str = "app.kubernetes.io/name";
/// A unique name identifying the instance of an application e.g. "mysql-abcxzy"
pub const APP_KUBERNETES_IO_INSTANCE: &str = "app.kubernetes.io/instance";
/// The current version of the application (e.g., a semantic version, revision hash, etc.) e.g."5.7.21"
pub const APP_KUBERNETES_IO_VERSION: &str = "app.kubernetes.io/version";
///	The component within the architecture e.g. database
pub const APP_KUBERNETES_IO_COMPONENT: &str = "app.kubernetes.io/component";
/// The name of a higher level application this one is part of e.g. "wordpress"
pub const APP_KUBERNETES_IO_PART_OF: &str = "app.kubernetes.io/part-of";
/// The tool being used to manage the operation of an application e.g. helm
pub const APP_KUBERNETES_IO_MANAGED_BY: &str = "app.kubernetes.io/managed-by";

/// Builds a `ObjectMeta` object out of a template/owner object.
///
/// Automatically sets:
/// * name
/// * namespace (if the object passed in had one)
/// * labels (if provided)
/// * kubernetes recommended labels e.g. app.kubernetes.io/instance
/// * ownerReferences (pointing at the object that was passed in).
pub fn build_metadata<T>(
    name: String,
    labels: Option<BTreeMap<String, String>>,
    resource: &T,
    block_owner_deletion: bool,
) -> OperatorResult<ObjectMeta>
where
    T: Meta,
{
    let mut merged_labels = get_recommended_labels(resource)?;

    if let Some(provided_labels) = labels {
        merged_labels.extend(provided_labels);
    }

    Ok(ObjectMeta {
        labels: Some(merged_labels),
        name: Some(name),
        namespace: Meta::namespace(resource),
        owner_references: Some(vec![object_to_owner_reference::<T>(
            resource.meta().clone(),
            block_owner_deletion,
        )?]),
        ..ObjectMeta::default()
    })
}

/// Create the kubernetes recommended labels:
/// - app.kubernetes.io/name - The name of the application	e.g. mysql
/// - app.kubernetes.io/instance - A unique name identifying the instance of an application e.g. mysql-abcxzy
/// - app.kubernetes.io/version	- The current version of the application (e.g., a semantic version, revision hash, etc.) e.g. 5.7.21
/// - app.kubernetes.io/component - The component within the architecture e.g. database
/// - app.kubernetes.io/part-of	- The name of a higher level application this one is part of e.g. wordpress
/// - app.kubernetes.io/managed-by - The tool being used to manage the operation of an application e.g. helm
///
fn get_recommended_labels<T>(resource: &T) -> OperatorResult<BTreeMap<String, String>>
where
    T: Meta,
{
    let mut recommended_labels = BTreeMap::new();
    // TODO: throw error if no name found? Can that even happen?
    if let Some(name) = &resource.meta().name {
        recommended_labels.insert(APP_KUBERNETES_IO_INSTANCE.to_string(), name.clone());
    }

    Ok(recommended_labels)
}

/// Creates an OwnerReference pointing to the resource type and `metadata` being passed in.
/// The created OwnerReference has it's `controller` flag set to `true`
pub fn object_to_owner_reference<K: Resource>(
    meta: ObjectMeta,
    block_owner_deletion: bool,
) -> OperatorResult<OwnerReference> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        name: meta.name.ok_or(Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.ok_or(Error::MissingObjectKey {
            key: ".metadata.uid",
        })?,
        controller: Some(true),
        block_owner_deletion: Some(block_owner_deletion),
    })
}

#[cfg(test)]
mod tests {

    use super::*;

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
        assert_eq!(
            labels.get(APP_KUBERNETES_IO_INSTANCE),
            Some(&"foo_pod".to_string())
        );
        assert_eq!(labels.len(), 2);

        Ok(())
    }
}
