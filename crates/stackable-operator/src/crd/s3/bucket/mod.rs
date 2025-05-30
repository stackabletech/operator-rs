use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{crd::s3::connection::v1alpha1 as conn_v1alpha1, versioned::versioned};

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    pub mod v1alpha1 {
        pub use v1alpha1_impl::BucketError;
    }

    /// S3 bucket specification containing the bucket name and an inlined or referenced connection specification.
    /// Learn more on the [S3 concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/s3).
    #[versioned(k8s(
        group = "s3.stackable.tech",
        kind = "S3Bucket",
        plural = "s3buckets",
        crates(
            kube_core = "kube::core",
            k8s_openapi = "k8s_openapi",
            schemars = "schemars",
        ),
        namespaced
    ))]
    #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BucketSpec {
        /// The name of the S3 bucket.
        pub bucket_name: String,

        /// The definition of an S3 connection, either inline or as a reference.
        pub connection: conn_v1alpha1::InlineConnectionOrReference,
    }

    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    // TODO: This probably should be serde(untagged), but this would be a breaking change
    pub enum InlineBucketOrReference {
        Inline(BucketSpec),
        Reference(String),
    }

    /// Use this struct in your operator.
    pub struct ResolvedBucket {
        pub bucket_name: String,
        pub connection: conn_v1alpha1::ConnectionSpec,
    }
}

#[cfg(test)]
impl stackable_versioned::flux_converter::test_utils::RoundtripTestData for v1alpha1::BucketSpec {
    fn get_roundtrip_test_data() -> Vec<Self> {
        crate::utils::yaml_from_str_singleton_map(indoc::indoc! {"
          - bucketName: my-example-bucket
            connection:
              reference: my-connection-resource
          - bucketName: foo
            connection:
              inline:
                host: s3.example.com
          - bucketName: foo
            connection:
              inline:
                host: s3.example.com
                port: 1234
                accessStyle: VirtualHosted
                credentials:
                  secretClass: s3-credentials
                region:
                  name: eu-west-1
                tls:
                  verification:
                    server:
                      caCert:
                        secretClass: s3-cert
        "})
        .expect("Failed to parse BucketSpec YAML")
    }
}
