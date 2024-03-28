use std::marker::PhantomData;

use derivative::Derivative;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::yaml;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const DOCS_HOME_URL_PLACEHOLDER: &str = "DOCS_BASE_URL_PLACEHOLDER";
const DOCS_HOME_BASE_URL: &str = "https://docs.stackable.tech/home";

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("cannot parse version {version:?} as a semantic version"))]
    InvalidSemverVersion {
        source: semver::Error,
        version: String,
    },

    #[snafu(display("error converting CRD byte array to UTF-8"))]
    CrdFromUtf8 { source: std::string::FromUtf8Error },

    #[snafu(display("failed to serialize YAML"))]
    YamlSerialization { source: yaml::Error },

    #[snafu(display("failed to write YAML"))]
    WriteYamlSchema { source: std::io::Error },

    #[snafu(display("failed to create YAML file"))]
    CreateYamlFile { source: std::io::Error },
}

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

/// Takes an operator version and returns a docs version
fn docs_version(operator_version: &str) -> Result<String> {
    if operator_version == "0.0.0-dev" {
        Ok("nightly".to_owned())
    } else {
        let v = Version::parse(operator_version).context(InvalidSemverVersionSnafu {
            version: operator_version.to_owned(),
        })?;
        Ok(format!("{}.{}", v.major, v.minor))
    }
}

/// Given an operator version like 0.0.0-dev or 23.1.1, generate a docs home
/// component base URL like `https://docs.stackable.tech/home/nightly/` or
/// `https://docs.stackable.tech/home/23.1/`.
fn docs_home_versioned_base_url(operator_version: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        DOCS_HOME_BASE_URL,
        docs_version(operator_version)?
    ))
}

/// This trait can be implemented to allow automatic handling
/// (e.g. creation) of `CustomResourceDefinition`s in Kubernetes.
pub trait CustomResourceExt: kube::CustomResourceExt {
    /// Generates a YAML CustomResourceDefinition and writes it to a `Write`.
    ///
    /// The generated YAML string is an explicit document with leading dashes (`---`).
    fn generate_yaml_schema<W>(mut writer: W, operator_version: &str) -> Result<()>
    where
        W: Write,
    {
        let mut buffer = Vec::new();
        yaml::serialize_to_explicit_document(&mut buffer, &Self::crd())
            .context(YamlSerializationSnafu)?;

        let yaml_schema = String::from_utf8(buffer)
            .context(CrdFromUtf8Snafu)?
            .replace(
                DOCS_HOME_URL_PLACEHOLDER,
                &docs_home_versioned_base_url(operator_version)?,
            );

        writer
            .write_all(yaml_schema.as_bytes())
            .context(WriteYamlSchemaSnafu)
    }

    /// Generates a YAML CustomResourceDefinition and writes it to the specified file.
    ///
    /// The written YAML string is an explicit document with leading dashes (`---`).
    fn write_yaml_schema<P: AsRef<Path>>(path: P, operator_version: &str) -> Result<()> {
        let writer = File::create(path).context(CreateYamlFileSnafu)?;
        Self::generate_yaml_schema(writer, operator_version)
    }

    /// Generates a YAML CustomResourceDefinition and prints it to stdout.
    ///
    /// The printed YAML string is an explicit document with leading dashes (`---`).
    fn print_yaml_schema(operator_version: &str) -> Result<()> {
        let writer = std::io::stdout();
        Self::generate_yaml_schema(writer, operator_version)
    }

    /// Returns the YAML schema of this CustomResourceDefinition as a string.
    ///
    /// The written YAML string is an explicit document with leading dashes (`---`).
    fn yaml_schema(operator_version: &str) -> Result<String> {
        let mut writer = Vec::new();
        Self::generate_yaml_schema(&mut writer, operator_version)?;
        String::from_utf8(writer).context(CrdFromUtf8Snafu)
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
