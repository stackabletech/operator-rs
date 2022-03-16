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
//! use stackable_operator::opa::OpaConfig;
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
//! assert_eq!(opa_config.package_url(&cluster), "v1/data/test".to_string());
//! assert_eq!(opa_config.full_package_url(&cluster, "http://localhost:8081"), "http://localhost:8081/v1/data/test".to_string());
//! assert_eq!(opa_config.rule_url(&cluster, Some("myrule")), "v1/data/test/myrule".to_string());
//! assert_eq!(opa_config.full_rule_url(&cluster, "http://localhost:8081", Some("myrule")), "http://localhost:8081/v1/data/test/myrule".to_string());
//! ```
use crate::client::Client;
use crate::error;
use crate::error::OperatorResult;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

const OPA_API: &str = "v1/data";

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaConfig {
    pub config_map_name: String,
    pub package: Option<String>,
}

impl OpaConfig {
    /// Returns the OPA data API url up to the package. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// This may be used if the OPA base url is contained in an ENV variable.
    ///
    /// # Example
    ///
    /// v1/data/<package>
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource.
    pub fn package_url<T>(&self, resource: &T) -> String
    where
        T: ResourceExt,
    {
        let package_name = match &self.package {
            Some(p) => p.to_string(),
            None => resource.name(),
        };

        format!("{}/{}", OPA_API, package_name)
    }

    /// Returns the OPA data API url up to the rule. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// This may be used if the OPA base url is contained in an ENV variable.
    ///
    /// # Example
    ///
    /// v1/data/<package>/<rule>
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource.
    /// * `rule`         - The rule name. Defaults to `allow`.
    pub fn rule_url<T>(&self, resource: &T, rule: Option<&str>) -> String
    where
        T: ResourceExt,
    {
        format!("{}/{}", self.package_url(resource), rule.unwrap_or("allow"))
    }

    /// Returns the full qualified OPA data API url up to the package. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// # Example
    ///
    /// http://localhost:8080/v1/data/<package>
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource
    /// * `opa_base_url` - The base url to OPA e.g. http://localhost:8081
    pub fn full_package_url<T>(&self, resource: &T, opa_base_url: &str) -> String
    where
        T: ResourceExt,
    {
        if opa_base_url.ends_with('/') {
            format!("{}{}", opa_base_url, self.package_url(resource))
        } else {
            format!("{}/{}", opa_base_url, self.package_url(resource))
        }
    }

    /// Returns the full qualified OPA data API url up to the rule. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// # Example
    ///
    /// http://localhost:8080/v1/data/<package>/<rule>
    ///
    /// # Arguments
    /// * `resource`     - The cluster resource.
    /// * `opa_base_url` - The base url to OPA e.g. http://localhost:8081.
    /// * `rule`         - The rule name. Defaults to `allow`.
    pub fn full_rule_url<T>(&self, resource: &T, opa_base_url: &str, rule: Option<&str>) -> String
    where
        T: ResourceExt,
    {
        if opa_base_url.ends_with('/') {
            format!("{}{}", opa_base_url, self.rule_url(resource, rule))
        } else {
            format!("{}/{}", opa_base_url, self.rule_url(resource, rule))
        }
    }

    /// Returns the full qualified OPA data API url up to the package. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// In contrast to `full_package_url`, this queries the OPA base url from the provided
    /// `config_map_name` in the [`OpaConfig`].
    ///
    /// # Example
    ///
    /// http://localhost:8080/v1/data/<package>
    ///
    /// # Arguments
    /// * `client`       - The kubernetes client.
    /// * `resource`     - The cluster resource.
    pub async fn full_package_url_from_config_map<T>(
        &self,
        client: &Client,
        resource: &T,
    ) -> OperatorResult<String>
    where
        T: ResourceExt,
    {
        let opa_base_url = self
            .base_url_from_config_map(client, resource.namespace().as_deref())
            .await?;

        Ok(self.full_package_url(resource, &opa_base_url))
    }

    /// Returns the full qualified OPA data API url up to the rule. If [`OpaConfig`] has
    /// no `package` set, will default to the cluster `resource` name.
    ///
    /// In contrast to `full_rule_url`, this queries the OPA base url from the provided
    /// `config_map_name` in the [`OpaConfig`].
    ///
    /// # Example
    ///
    /// http://localhost:8080/v1/data/<package>/<rule>
    ///
    /// # Arguments
    /// * `client`       - The kubernetes client.
    /// * `resource`     - The cluster resource.
    /// * `rule`         - The rule name. Defaults to `allow`.
    pub async fn full_rule_url_from_config_map<T>(
        &self,
        client: &Client,
        resource: &T,
        rule: Option<&str>,
    ) -> OperatorResult<String>
    where
        T: ResourceExt,
    {
        let opa_base_url = self
            .base_url_from_config_map(client, resource.namespace().as_deref())
            .await?;

        Ok(self.full_rule_url(resource, &opa_base_url, rule))
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
        Ok(client
            .get::<ConfigMap>(&self.config_map_name, namespace)
            .await?
            .data
            .and_then(|mut data| data.remove("OPA"))
            .ok_or(error::Error::MissingOpaConnectString {
                configmap_name: self.config_map_name.clone(),
            })?)
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
    const RULE_DEFAULT: &str = "allow";
    const RULE_NAME: &str = "test-rule";
    const OPA_BASE_URL_WITH_SLASH: &str = "http://opa:8081/";
    const OPA_BASE_URL_WITHOUT_SLASH: &str = "http://opa:8081";

    #[test]
    fn test_package_url_with_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(Some(PACKAGE_NAME));

        assert_eq!(
            opa_config.package_url(&cluster),
            format!("{}/{}", OPA_API, PACKAGE_NAME)
        )
    }

    #[test]
    fn test_package_url_without_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.package_url(&cluster),
            format!("{}/{}", OPA_API, CLUSTER_NAME)
        )
    }

    #[test]
    fn test_rule_url_with_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(Some(PACKAGE_NAME));

        assert_eq!(
            opa_config.rule_url(&cluster, None),
            format!("{}/{}/{}", OPA_API, PACKAGE_NAME, RULE_DEFAULT)
        );

        assert_eq!(
            opa_config.rule_url(&cluster, Some(RULE_NAME)),
            format!("{}/{}/{}", OPA_API, PACKAGE_NAME, RULE_NAME)
        );
    }

    #[test]
    fn test_rule_url_without_package_name() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.rule_url(&cluster, None),
            format!("{}/{}/{}", OPA_API, CLUSTER_NAME, RULE_DEFAULT)
        );

        assert_eq!(
            opa_config.rule_url(&cluster, Some(RULE_NAME)),
            format!("{}/{}/{}", OPA_API, CLUSTER_NAME, RULE_NAME)
        );
    }

    #[test]
    fn test_full_package_url() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.full_package_url(&cluster, OPA_BASE_URL_WITH_SLASH),
            format!("{}{}/{}", OPA_BASE_URL_WITH_SLASH, OPA_API, CLUSTER_NAME)
        );

        let opa_config = build_opa_config(Some(PACKAGE_NAME));

        assert_eq!(
            opa_config.full_package_url(&cluster, OPA_BASE_URL_WITHOUT_SLASH),
            format!(
                "{}/{}/{}",
                OPA_BASE_URL_WITHOUT_SLASH, OPA_API, PACKAGE_NAME
            )
        );
    }

    #[test]
    fn test_full_rule_url() {
        let cluster = build_test_cluster();
        let opa_config = build_opa_config(None);

        assert_eq!(
            opa_config.full_rule_url(&cluster, OPA_BASE_URL_WITHOUT_SLASH, None),
            format!(
                "{}/{}/{}/{}",
                OPA_BASE_URL_WITHOUT_SLASH, OPA_API, CLUSTER_NAME, RULE_DEFAULT
            )
        );

        assert_eq!(
            opa_config.full_rule_url(&cluster, OPA_BASE_URL_WITHOUT_SLASH, Some(RULE_NAME)),
            format!(
                "{}/{}/{}/{}",
                OPA_BASE_URL_WITHOUT_SLASH, OPA_API, CLUSTER_NAME, RULE_NAME
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
}
