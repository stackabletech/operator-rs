use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::commons::{secret_class::SecretClassVolume, tls::TlsClientDetails};

use super::S3ConnectionInlineOrReference;

/// Contains connection and access details to access an S3 object store.
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
    /// Hostname of the S3 server without any protocol or port.
    // TODO: Rename to `hostname` to be more consistent with other structs.
    #[serde(rename = "host")]
    pub hostname: String,

    /// Port the S3 server listens on.
    /// Port of the S3 server. If TLS is used defaults to 443 otherwise to 80.
    pub(crate) port: Option<u16>,

    /// Which access style to use.
    /// Defaults to virtual hosted-style as most of the data products out there.
    /// Have a look at the official documentation on <https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html>
    #[serde(default)]
    pub access_style: S3AccessStyle,

    /// If the S3 uses authentication you have to specify you S3 credentials.
    /// In the most cases a SecretClass providing `accessKey` and `secretKey` is sufficient.
    pub credentials: Option<SecretClassVolume>,

    /// If you want to use TLS when talking to S3 you can enable TLS encrypted communication with this setting.
    #[serde(flatten)]
    pub tls: TlsClientDetails,
}

/// Contains the name of the bucket as well as the needed connection details.
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
    /// Name of the bucket
    pub(crate) bucket_name: String,
    /// Either a inlined s3 connection or a reference to a S3Connection object
    pub(crate) connection: S3ConnectionInlineOrReference,
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
