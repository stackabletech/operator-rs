//! This module offers common access to the [`OpaConfig`] which can be used in operators
//! to specify a name for a [`k8s_openapi::api::core::v1::ConfigMap`] and a package name
//! for OPA rules.
//!
//! Additionally several methods are provided to build an URL to query the OPA data API.
//!
//! # Example
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use stackable_operator::kube::CustomResource;
//! use stackable_operator::commons::opa::{OpaApiVersion, OpaConfig};
//! use stackable_operator::schemars::{self, JsonSchema};
//!
//! #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
//! #[kube(
//!     group = "test.stackable.tech",
//!     version = "v1alpha1",
//!     kind = "TestCluster",
//!     plural = "testclusters",
//!     shortname = "test",
//!     namespaced,
//! )]
//! #[serde(rename_all = "camelCase")]
//! pub struct TestClusterSpec {
//!     opa: Option<OpaConfig>    
//! }
//!
//! let cluster: TestCluster = serde_yaml::from_str(
//!     "
//!     apiVersion: test.stackable.tech/v1alpha1
//!     kind: TestCluster
//!     metadata:
//!       name: simple-test
//!     spec:
//!       opa:
//!         configMapName: simple-opa
//!         package: test
//!     ",
//!     ).unwrap();
//!
//! let opa_config: &OpaConfig = cluster.spec.opa.as_ref().unwrap();
//!
//! assert_eq!(opa_config.document_url(&cluster, Some("allow"), OpaApiVersion::V1), "v1/data/test/allow".to_string());
//! assert_eq!(opa_config.full_document_url(&cluster, "http://localhost:8081", None, OpaApiVersion::V1), "http://localhost:8081/v1/data/test".to_string());
//! ```
use crate::client::Client;
use crate::error;
use crate::error::OperatorResult;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use lazy_static::lazy_static;
use regex::Regex;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref DOT_REGEX: Regex = Regex::new("\\.").unwrap();
    /// To remove leading slashes from OPA package name (if present)
    static ref LEADING_SLASH_REGEX: Regex = Regex::new("(/*)(.*)").unwrap();
}
/// Indicates the OPA API version. This is required to choose the correct
/// path when constructing the OPA urls to query.
pub enum OpaApiVersion {
    V1,
}

impl OpaApiVersion {
    /// Returns the OPA data API path for the selected version
    pub fn get_data_api(&self) -> &'static str {
        match self {
            Self::V1 => "v1/data",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaConfig {
    pub config_map_name: String,
    pub package: Option<String>,
}

impl OpaConfig {
    /// Returns the OPA data API url. If [`OpaConfig`] has no `package` set,
    /// will default to the cluster `resource` name.
    ///
    /// The rule is optional and will be appended to the `<package>` part if
    /// provided as can be seen in the examples below.
    ///
    /// This may be used if the OPA base url is contained in an ENV variable.
    ///
    /// # Example
    ///
    /// * `v1/data/<package>`
    /// * `v1/data/<package>/<rule>`
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource.
    /// * `rule`         - The rule name. Can be omitted.
    /// * `api_version`  - The [`OpaApiVersion`] to extract the data API path.
    pub fn document_url<T>(
        &self,
        resource: &T,
        rule: Option<&str>,
        api_version: OpaApiVersion,
    ) -> String
    where
        T: ResourceExt,
    {
        let package_name = match &self.package {
            Some(p) => Self::sanitize_opa_package_name(p),
            None => resource.name_any(),
        };

        let mut document_url = format!("{}/{}", api_version.get_data_api(), package_name);

        if let Some(document_rule) = rule {
            document_url.push('/');
            document_url.push_str(document_rule);
        }

        document_url
    }

    /// Returns the full qualified OPA data API url. If [`OpaConfig`] has no `package` set,
    /// will default to the cluster `resource` name.
    ///
    /// The rule is optional and will be appended to the `<package>` part if
    /// provided as can be seen in the examples below.
    ///
    /// # Example
    ///
    /// * `http://localhost:8081/v1/data/<package>`
    /// * `http://localhost:8081/v1/data/<package>/<rule>`
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource
    /// * `opa_base_url` - The base url to OPA e.g. http://localhost:8081
    /// * `rule`         - The rule name. Can be omitted.
    /// * `api_version`  - The [`OpaApiVersion`] to extract the data API path.
    pub fn full_document_url<T>(
        &self,
        resource: &T,
        opa_base_url: &str,
        rule: Option<&str>,
        api_version: OpaApiVersion,
    ) -> String
    where
        T: ResourceExt,
    {
        if opa_base_url.ends_with('/') {
            format!(
                "{}{}",
                opa_base_url,
                self.document_url(resource, rule, api_version)
            )
        } else {
            format!(
                "{}/{}",
                opa_base_url,
                self.document_url(resource, rule, api_version)
            )
        }
    }

    /// Returns the full qualified OPA data API url up to the package. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// The rule is optional and will be appended to the `<package>` part if
    /// provided as can be seen in the examples below.
    ///
    /// In contrast to `full_document_url`, this extracts the OPA base url from the provided
    /// `config_map_name` in the [`OpaConfig`].
    ///
    /// # Example
    ///
    /// * `http://localhost:8081/v1/data/<package>`
    /// * `http://localhost:8081/v1/data/<package>/<rule>`
    ///
    /// # Arguments
    /// * `client`       - The kubernetes client.
    /// * `resource`     - The cluster resource.
    /// * `rule`         - The rule name. Can be omitted.
    /// * `api_version`  - The [`OpaApiVersion`] to extract the data API path.
    pub async fn full_document_url_from_config_map<T>(
        &self,
        client: &Client,
        resource: &T,
        rule: Option<&str>,
        api_version: OpaApiVersion,
    ) -> OperatorResult<String>
    where
        T: ResourceExt,
    {
        let opa_base_url = self
            .base_url_from_config_map(client, resource.namespace().as_deref())
            .await?;

        Ok(self.full_document_url(resource, &opa_base_url, rule, api_version))
    }

    /// Returns the OPA base url defined in the [`k8s_openapi::api::core::v1::ConfigMap`]
    /// from `config_map_name` in the [`OpaConfig`].
    ///
    /// # Arguments
    /// * `client`       - The kubernetes client.
    /// * `namespace`    - The namespace of the config map.
    async fn base_url_from_config_map(
        &self,
        client: &Client,
        namespace: Option<&str>,
    ) -> OperatorResult<String> {
        client
            .get::<ConfigMap>(&self.config_map_name, namespace)
            .await?
            .data
            .and_then(|mut data| data.remove("OPA"))
            .ok_or(error::Error::MissingOpaConnectString {
                configmap_name: self.config_map_name.clone(),
            })
    }

    /// Removes leading slashes from OPA package name. Dots are converted to forward slashes.
    ///
    /// # Arguments
    /// * `package_name`    - Package name to sanitize
    fn sanitize_opa_package_name(package_name: &str) -> String {
        // Package names starting with one or more slashes cause the resulting URL to be invalid, hence removed.
        let no_leading_slashes = LEADING_SLASH_REGEX.replace_all(package_name, "$2");
        // All dots must be replaced with forward slashes in order for the URL to be a valid resource
        DOT_REGEX.replace_all(&no_leading_slashes, "/").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResource;
    use schemars::{self, JsonSchema};
    use serde::{Deserialize, Serialize};

    const CLUSTER_NAME: &str = "simple-cluster";
    const PACKAGE_NAME: &str = "my-package";
    const RULE_NAME: &str = "allow";
    const OPA_BASE_URL_WITH_SLASH: &str = "http://opa:8081/";
    const OPA_BASE_URL_WITHOUT_SLASH: &str = "http://opa:8081";

    const V1: OpaApiVersion = OpaApiVersion::V1;

    #[test]
    fn test_document_url_with_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(Some(PACKAGE_NAME));

        assert_eq!(
            opa_config.document_url(&cluster, None, V1),
            format!("{}/{}", V1.get_data_api(), PACKAGE_NAME)
        );

        assert_eq!(
            opa_config.document_url(&cluster, Some(RULE_NAME), V1),
            format!("{}/{}/{}", V1.get_data_api(), PACKAGE_NAME, RULE_NAME)
        );
    }

    #[test]
    fn test_document_url_without_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.document_url(&cluster, None, V1),
            format!("{}/{}", V1.get_data_api(), CLUSTER_NAME)
        );

        assert_eq!(
            opa_config.document_url(&cluster, Some(RULE_NAME), V1),
            format!("{}/{}/{}", V1.get_data_api(), CLUSTER_NAME, RULE_NAME)
        );
    }

    #[test]
    fn test_full_document_url() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.full_document_url(&cluster, OPA_BASE_URL_WITH_SLASH, None, V1),
            format!(
                "{}/{}/{}",
                OPA_BASE_URL_WITHOUT_SLASH,
                V1.get_data_api(),
                CLUSTER_NAME
            )
        );

        let opa_config = build_opa_config(Some(PACKAGE_NAME));

        assert_eq!(
            opa_config.full_document_url(&cluster, OPA_BASE_URL_WITHOUT_SLASH, None, V1),
            format!(
                "{}/{}/{}",
                OPA_BASE_URL_WITHOUT_SLASH,
                V1.get_data_api(),
                PACKAGE_NAME
            )
        );
    }

    #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[kube(group = "test", version = "v1", kind = "TestCluster", namespaced)]
    pub struct ClusterSpec {
        test: u8,
    }

    fn build_test_cluster() -> TestCluster {
        serde_yaml::from_str(&format!(
            "
            apiVersion: test/v1
            kind: TestCluster
            metadata:
              name: {}
            spec:
              test: 100
            ",
            CLUSTER_NAME
        ))
        .unwrap()
    }

    fn build_opa_config(package: Option<&str>) -> OpaConfig {
        OpaConfig {
            config_map_name: "opa".to_string(),
            package: package.map(|p| p.to_string()),
        }
    }

    #[test]
    fn test_opa_package_name_sanitizer() {
        // No sanitization needed
        assert_eq!(
            OpaConfig::sanitize_opa_package_name("kafka/authz"),
            "kafka/authz"
        );

        // Remove single leading slash and convert dot to slash
        assert_eq!(
            OpaConfig::sanitize_opa_package_name("/kafka.authz"),
            "kafka/authz"
        );

        // Remove multiple leading slashes and convert dot
        assert_eq!(
            OpaConfig::sanitize_opa_package_name("////kafka.authz"),
            "kafka/authz"
        );
    }

    #[test]
    fn test_opa_document_url_sanitization() {
        let opa_config = OpaConfig {
            config_map_name: "simple-opa".to_owned(),
            package: Some("///kafka.authz".to_owned()),
        };

        let document_url = opa_config.document_url(
            &k8s_openapi::api::core::v1::Pod::default(),
            None,
            OpaApiVersion::V1,
        );
        assert_eq!(document_url, "v1/data/kafka/authz")
    }
}
