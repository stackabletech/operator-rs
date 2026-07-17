use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{commons::tls_verification::TlsClientDetails, versioned::versioned};

mod v1alpha1_impl;

// FIXME (@Techassi): This should be versioned as well, but the macro cannot
// handle new-type structs yet.
/// Use this type in your operator!
pub type ResolvedOpenLineageConnection = v1alpha1::OpenLineageConnectionSpec;

#[versioned(
    version(name = "v1alpha1"),
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars",
    )
)]
pub mod versioned {
    pub mod v1alpha1 {
        pub use v1alpha1_impl::OpenLineageError;
    }

    /// OpenLineage connection definition as a resource.
    /// Learn more about [OpenLineage](https://openlineage.io/).
    #[versioned(crd(
        group = "openlineage.stackable.tech",
        kind = "OpenLineageConnection",
        plural = "openlineageconnections",
        doc = "A reusable definition of a connection to an OpenLineage backend.",
        namespaced
    ))]
    #[derive(CustomResource, Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenLineageConnectionSpec {
        /// Host of the OpenLineage backend without any protocol or port. For example: `marquez`.
        pub host: String,

        /// Port the OpenLineage backend listens on. For example: `5000`.
        pub port: u16,

        /// Use a TLS connection. If not specified no TLS will be used.
        /// When TLS server verification is configured, the transport uses `https` instead of `http`.
        #[serde(flatten)]
        pub tls: TlsClientDetails,
    }

    /// An OpenLineage connection, either inlined or referenced by the name of an
    /// [`OpenLineageConnection`] resource in the same namespace.
    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    // TODO: This probably should be serde(untagged), but this would be a breaking change
    pub enum InlineConnectionOrReference {
        Inline(OpenLineageConnectionSpec),
        Reference(String),
    }

    /// OpenLineage lineage-emission configuration for a single workload/application.
    ///
    /// Embed this in an operator's workload spec to enable OpenLineage for that workload.
    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenLineageJob {
        /// The OpenLineage backend connection, either inlined or referencing an
        /// `OpenLineageConnection` resource by name.
        pub connection: InlineConnectionOrReference,

        /// The OpenLineage namespace lineage is reported under.
        /// If unset, operators typically default to the workload's Kubernetes namespace.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub namespace: Option<String>,

        /// A stable OpenLineage job/application name. Setting this prevents fragmented run history.
        /// If unset, operators resolve a name from workload-specific configuration.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub app_name: Option<String>,
    }
}

#[cfg(test)]
impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::OpenLineageConnectionSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        crate::utils::yaml_from_str_singleton_map(indoc::indoc! {"
            - host: marquez
              port: 5000
            - host: marquez
              port: 5000
              tls:
                verification:
                  none: {}
            - host: marquez
              port: 5000
              tls:
                verification:
                  server:
                    caCert:
                      secretClass: openlineage-cert
        "})
        .expect("Failed to parse OpenLineageConnectionSpec YAML")
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        commons::tls_verification::{
            CaCert, Tls, TlsClientDetails, TlsServerVerification, TlsVerification,
        },
        crd::openlineage::v1alpha1::OpenLineageConnectionSpec,
    };

    #[test]
    fn http_transport_url_without_tls() {
        let connection = OpenLineageConnectionSpec {
            host: "marquez".to_string(),
            port: 5000,
            tls: TlsClientDetails { tls: None },
        };

        assert_eq!(connection.transport_url(), "http://marquez:5000");
    }

    #[test]
    fn https_transport_url_with_server_verification() {
        let connection = OpenLineageConnectionSpec {
            host: "marquez".to_string(),
            port: 5000,
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: TlsVerification::Server(TlsServerVerification {
                        ca_cert: CaCert::WebPki {},
                    }),
                }),
            },
        };

        assert_eq!(connection.transport_url(), "https://marquez:5000");
    }

    #[test]
    fn http_transport_url_without_verification() {
        let connection = OpenLineageConnectionSpec {
            host: "marquez".to_string(),
            port: 5000,
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: TlsVerification::None {},
                }),
            },
        };

        assert_eq!(connection.transport_url(), "http://marquez:5000");
    }
}
