use std::marker::PhantomData;

use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::OperatorResult;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// A reference to a product cluster (for example, a `ZookeeperCluster`)
///
/// `namespace`'s defaulting only applies when retrieved via [`ClusterRef::namespace_relative_from`]
#[derive(Deserialize, Serialize, JsonSchema, Derivative)]
#[derivative(
    Default(bound = ""),
    Clone(bound = ""),
    Debug(bound = ""),
    PartialEq(bound = "")
)]
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

/// This trait can be implemented to allow automatic handling
/// (e.g. creation) of `CustomResourceDefinition`s in Kubernetes.
pub trait CustomResourceExt: kube::CustomResourceExt {
    /// Generates a YAML CustomResourceDefinition and writes it to a `Write`.
    fn generate_yaml_schema<W>(mut writer: W) -> OperatorResult<()>
    where
        W: Write,
    {
        let schema = serde_yaml::to_string(&Self::crd())?;
        writer.write_all(schema.as_bytes())?;
        Ok(())
    }

    /// Generates a YAML CustomResourceDefinition and writes it to the specified file.
    fn write_yaml_schema<P: AsRef<Path>>(path: P) -> OperatorResult<()> {
        let writer = File::create(path)?;
        Self::generate_yaml_schema(writer)
    }

    /// Generates a YAML CustomResourceDefinition and prints it to stdout.
    fn print_yaml_schema() -> OperatorResult<()> {
        let writer = std::io::stdout();
        Self::generate_yaml_schema(writer)
    }

    // Returns the YAML schema of this CustomResourceDefinition as a string.
    fn yaml_schema() -> OperatorResult<String> {
        let mut writer = Vec::new();
        Self::generate_yaml_schema(&mut writer)?;
        Ok(String::from_utf8(writer)?)
    }
}

impl<T> CustomResourceExt for T where T: kube::CustomResourceExt {}

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
