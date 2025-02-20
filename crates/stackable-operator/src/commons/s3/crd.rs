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

    /// AWS service API region used by the AWS SDK when using AWS S3 buckets.
    ///
    /// This defaults to `us-east-1` and can be ignored if not using AWS S3
    /// buckets.
    ///
    /// NOTE: This is not the bucket region, and is used by the AWS SDK to
    /// construct endpoints for various AWS service APIs. It is only useful when
    /// using AWS S3 buckets.
    ///
    /// When using AWS S3 buckets, you can configure optimal AWS service API
    /// connections in the following ways:
    /// - From **inside** AWS: Use an auto-discovery source (eg: AWS IMDS).
    /// - From **outside** AWS, or when IMDS is disabled, explicity set the
    ///   region name nearest to where the client application is running from.
    #[serde(default)]
    pub region: AwsRegion,

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

/// Set a named AWS region, or defer to an auto-discovery mechanism.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AwsRegion {
    /// Defer region detection to an auto-discovery mechanism.
    Source(AwsRegionAutoDiscovery),

    /// An explicit region, eg: eu-central-1
    Name(String),
}

impl AwsRegion {
    /// Get the AWS region name.
    ///
    /// Returns `None` if an auto-discovery source has been selected. Otherwise,
    /// it returns the configured region name.
    ///
    /// Example usage:
    ///
    /// ```
    /// # use stackable_operator::commons::s3::AwsRegion;
    /// # fn set_property(key: &str, value: String) {}
    /// # fn example(aws_region: AwsRegion) {
    /// if let Some(region_name) = aws_region.name() {
    ///     // set some propery if the region is set, or is the default.
    ///     set_property("aws.region", region_name);
    /// };
    /// # }
    /// ```
    pub fn name(self) -> Option<String> {
        match self {
            AwsRegion::Name(name) => Some(name),
            AwsRegion::Source(_) => None,
        }
    }
}

impl Default for AwsRegion {
    fn default() -> Self {
        Self::Name("us-east-1".to_owned())
    }
}

/// AWS region auto-discovery mechanism.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum AwsRegionAutoDiscovery {
    /// AWS Instance Meta Data Service.
    ///
    /// This variant should result in no region being given to the AWS SDK,
    /// which should, in turn, query the AWS IMDS.
    AwsImds,
}
