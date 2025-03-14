use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::commons::{
    networking::HostName, s3::S3ConnectionInlineOrReference, secret_class::SecretClassVolume,
    tls_verification::TlsClientDetails,
};

/// S3 bucket specification containing the bucket name and an inlined or referenced connection specification.
/// Learn more on the [S3 concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/s3).
#[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "s3.stackable.tech",
    version = "v1alpha1",
    kind = "S3Bucket",
    plural = "s3buckets",
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    ),
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct S3BucketSpec {
    /// The name of the S3 bucket.
    pub bucket_name: String,

    /// The definition of an S3 connection, either inline or as a reference.
    pub connection: S3ConnectionInlineOrReference,
}

/// S3 connection definition as a resource.
/// Learn more on the [S3 concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/s3).
#[derive(CustomResource, Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "s3.stackable.tech",
    version = "v1alpha1",
    kind = "S3Connection",
    plural = "s3connections",
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    ),
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct S3ConnectionSpec {
    /// Host of the S3 server without any protocol or port. For example: `west1.my-cloud.com`.
    pub host: HostName,

    /// Port the S3 server listens on.
    /// If not specified the product will determine the port to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Bucket region used for signing headers (sigv4).
    ///
    /// This defaults to `us-east-1` which is compatible with other implementations such as Minio.
    ///
    /// WARNING: Some products use the Hadoop S3 implementation which falls back to us-east-2.
    #[serde(default)]
    pub region: Region,

    /// Which access style to use.
    /// Defaults to virtual hosted-style as most of the data products out there.
    /// Have a look at the [AWS documentation](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html).
    #[serde(default)]
    pub access_style: S3AccessStyle,

    /// If the S3 uses authentication you have to specify you S3 credentials.
    /// In the most cases a [SecretClass](DOCS_BASE_URL_PLACEHOLDER/secret-operator/secretclass)
    /// providing `accessKey` and `secretKey` is sufficient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<SecretClassVolume>,

    /// Use a TLS connection. If not specified no TLS will be used.
    #[serde(flatten)]
    pub tls: TlsClientDetails,
}

#[derive(
    strum::Display, Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize,
)]
#[strum(serialize_all = "PascalCase")]
pub enum S3AccessStyle {
    /// Use path-style access as described in <https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html#path-style-access>
    Path,

    /// Use as virtual hosted-style access as described in <https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html#virtual-hosted-style-access>
    #[default]
    VirtualHosted,
}

/// Set a named S3 Bucket region.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    #[serde(default = "Region::default_region_name")]
    pub name: String,
}

/// Having it as const &str as well, so we don't always allocate a [`String`] just for comparisons
const DEFAULT_REGION_NAME: &str = "us-east-1";
impl Region {
    fn default_region_name() -> String {
        DEFAULT_REGION_NAME.to_string()
    }

    /// Returns if the region sticks to the Stackable defaults.
    ///
    /// Some products don't really support configuring the region.
    /// This function can be used to determine if a warning or error should be raised to inform the
    /// user of this situation.
    pub fn is_default_config(&self) -> bool {
        self.name == DEFAULT_REGION_NAME
    }
}

impl Default for Region {
    fn default() -> Self {
        Self {
            name: Self::default_region_name(),
        }
    }
}
