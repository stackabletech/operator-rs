//! Implementation of the bucket definition as described in
//! the <https://github.com/stackabletech/documentation/pull/177>
//!
//! Operator CRDs are expected to use the [S3BucketDef] as an entry point to this module
//! and obtain an [InlinedS3BucketSpec] by calling [`S3BucketDef::resolve`].
//!
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{
    client::Client,
    commons::{authentication::tls::Tls, networking::Host, secret_class::SecretClassVolume},
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("missing S3Connection {resource_name:?} in namespace {namespace:?}"))]
    MissingS3Connection {
        source: crate::client::Error,
        resource_name: String,
        namespace: String,
    },

    #[snafu(display("missing S3Bucket {resource_name:?} in namespace {namespace:?}"))]
    MissingS3Bucket {
        source: crate::client::Error,
        resource_name: String,
        namespace: String,
    },
}

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
    pub connection: S3ConnectionDef,
}

impl S3BucketSpec {
    /// Convenience function to retrieve the spec of a S3 bucket resource from the K8S API service.
    pub async fn get(
        resource_name: &str,
        client: &Client,
        namespace: &str,
    ) -> Result<S3BucketSpec> {
        client
            .get::<S3Bucket>(resource_name, namespace)
            .await
            .map(|crd| crd.spec)
            .context(MissingS3BucketSnafu {
                resource_name,
                namespace,
            })
    }

    /// Map &self to an [InlinedS3BucketSpec] by obtaining connection spec from the K8S API service if necessary
    pub async fn inlined(&self, client: &Client, namespace: &str) -> Result<InlinedS3BucketSpec> {
        Ok(InlinedS3BucketSpec {
            connection: self.connection.resolve(client, namespace).await?,
            bucket_name: self.bucket_name.clone(),
        })
    }
}

/// Convenience struct with the connection spec inlined.
pub struct InlinedS3BucketSpec {
    /// Name of the S3 bucket
    pub bucket_name: String,

    // docs are on the struct
    pub connection: S3ConnectionSpec,
}

impl InlinedS3BucketSpec {
    /// Build the endpoint URL from [S3ConnectionSpec::host] and [S3ConnectionSpec::port] and the S3 implementation to use
    pub fn endpoint(&self) -> String {
        self.connection.endpoint()
    }
}

/// An S3 bucket definition, it can either be a reference to an explicit S3Bucket object,
/// or it can be an inline definition of a bucket. Read the
/// [S3 resources concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/s3)
/// to learn more.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum S3BucketDef {
    /// An inline definition, containing the S3 bucket properties.
    Inline(S3BucketSpec),
    /// A reference to an S3 bucket object. This is simply the name of the `S3Bucket`
    /// resource.
    Reference(String),
}

impl S3BucketDef {
    /// Returns an [InlinedS3BucketSpec].
    pub async fn resolve(&self, client: &Client, namespace: &str) -> Result<InlinedS3BucketSpec> {
        match self {
            S3BucketDef::Inline(s3_bucket) => s3_bucket.inlined(client, namespace).await,
            S3BucketDef::Reference(s3_bucket) => {
                S3BucketSpec::get(s3_bucket.as_str(), client, namespace)
                    .await?
                    .inlined(client, namespace)
                    .await
            }
        }
    }
}

/// Operators are expected to define fields for this type in order to work with S3 connections.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum S3ConnectionDef {
    /// Inline definition of an S3 connection.
    Inline(S3ConnectionSpec),
    /// A reference to an S3Connection resource.
    Reference(String),
}

impl S3ConnectionDef {
    /// Returns an [S3ConnectionSpec].
    pub async fn resolve(&self, client: &Client, namespace: &str) -> Result<S3ConnectionSpec> {
        match self {
            S3ConnectionDef::Inline(s3_connection_spec) => Ok(s3_connection_spec.clone()),
            S3ConnectionDef::Reference(s3_conn_reference) => {
                S3ConnectionSpec::get(s3_conn_reference, client, namespace).await
            }
        }
    }
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
    pub host: Host,

    /// Port the S3 server listens on.
    /// If not specified the product will determine the port to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

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

    /// If you want to use TLS when talking to S3 you can enable TLS encrypted communication with this setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<Tls>,
}

impl S3ConnectionSpec {
    /// Convenience function to retrieve the spec of a S3 connection resource from the K8S API service.
    pub async fn get(
        resource_name: &str,
        client: &Client,
        namespace: &str,
    ) -> Result<S3ConnectionSpec> {
        client
            .get::<S3Connection>(resource_name, namespace)
            .await
            .map(|conn| conn.spec)
            .context(MissingS3ConnectionSnafu {
                resource_name,
                namespace,
            })
    }

    /// Build the endpoint URL from this connection
    pub fn endpoint(&self) -> String {
        let protocol = match self.tls.as_ref() {
            Some(_tls) => "https",
            _ => "http",
        };
        match self.port {
            Some(p) => format!("{protocol}://{host}:{p}", host = self.host),
            None => format!("{protocol}://{host}", host = self.host),
        }
    }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ser_inline() {
        let bucket = S3BucketSpec {
            bucket_name: "test-bucket-name".to_owned(),
            connection: S3ConnectionDef::Inline(S3ConnectionSpec {
                host: "host".to_string().try_into().unwrap(),
                port: Some(8080),
                credentials: None,
                access_style: S3AccessStyle::VirtualHosted,
                tls: None,
            }),
        };

        let mut buf = Vec::new();
        crate::yaml::serialize_to_explicit_document(&mut buf, &bucket).expect("serializable value");
        let actual_yaml = std::str::from_utf8(&buf).expect("UTF-8 encoded document");

        let expected_yaml = "---
bucketName: test-bucket-name
connection:
  inline:
    host: host
    port: 8080
    accessStyle: VirtualHosted
";

        assert_eq!(expected_yaml, actual_yaml)
    }
}
