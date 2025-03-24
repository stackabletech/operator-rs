use std::marker::PhantomData;

use educe::Educe;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod authentication;
pub mod listener;
pub mod s3;

/// A reference to a product cluster (for example, a `ZookeeperCluster`)
///
/// `namespace`'s defaulting only applies when retrieved via [`ClusterRef::namespace_relative_from`]
#[derive(Deserialize, Serialize, JsonSchema, Educe)]
#[educe(Clone(bound()), Debug(bound()), Default(bound()), PartialEq(bound()))]
pub struct ClusterRef<K> {
    /// The name of the cluster
    pub name: Option<String>,

    /// The namespace of the cluster
    ///
    /// This field is optional, and will default to the namespace of the referring object.
    #[serde(default)]
    pub namespace: Option<String>,

    #[serde(skip)]
    _kind: PhantomData<K>,
}

impl<K: kube::Resource> ClusterRef<K> {
    pub fn to_named(name: &str, namespace: Option<&str>) -> Self {
        Self {
            name: Some(name.into()),
            namespace: namespace.map(|ns| ns.into()),
            _kind: PhantomData,
        }
    }

    pub fn to_object(obj: &K) -> Self {
        Self {
            name: obj.meta().name.clone(),
            namespace: obj.meta().namespace.clone(),
            _kind: PhantomData,
        }
    }

    pub fn namespace_relative_from<'a, K2: kube::Resource>(
        &'a self,
        container: &'a K2,
    ) -> Option<&'a str> {
        self.namespace
            .as_deref()
            .or_else(|| container.meta().namespace.as_deref())
    }
}

/// Retrieve the custom resource name (e.g. simple-test-cluster).
pub trait HasInstance {
    fn get_instance_name(&self) -> &str;
}

/// Retrieve the application name (e.g. spark, zookeeper).
pub trait HasApplication {
    fn get_application_name() -> &'static str;
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::ConfigMap;
    use kube::core::ObjectMeta;

    use super::ClusterRef;

    #[test]
    fn cluster_ref_should_default_namespace() {
        let relative_ref = ClusterRef::<ConfigMap>::to_named("foo", None);
        let absolute_ref = ClusterRef::<ConfigMap>::to_named("foo", Some("bar"));

        let nsless_obj = ConfigMap::default();
        let namespaced_obj = ConfigMap {
            metadata: ObjectMeta {
                namespace: Some("baz".to_string()),
                ..ObjectMeta::default()
            },
            ..ConfigMap::default()
        };

        assert_eq!(relative_ref.namespace_relative_from(&nsless_obj), None);
        assert_eq!(
            absolute_ref.namespace_relative_from(&nsless_obj),
            Some("bar")
        );
        assert_eq!(
            relative_ref.namespace_relative_from(&namespaced_obj),
            Some("baz")
        );
        assert_eq!(
            absolute_ref.namespace_relative_from(&namespaced_obj),
            Some("bar")
        );
    }
}
