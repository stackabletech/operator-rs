use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt as _, Snafu};

use crate::{
    client::Client,
    crd::s3::{ConnectionError, ConnectionInlineOrReference, ConnectionSpec},
};

#[derive(Debug, Snafu)]
pub enum BucketError {
    #[snafu(display("failed to retrieve S3 connection '{s3_connection}'"))]
    RetrieveS3Connection {
        source: crate::client::Error,
        s3_connection: String,
    },

    #[snafu(display("failed to resolve S3 connection"))]
    ResolveConnection { source: ConnectionError },
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
pub struct BucketSpec {
    /// The name of the S3 bucket.
    pub bucket_name: String,

    /// The definition of an S3 connection, either inline or as a reference.
    pub connection: ConnectionInlineOrReference,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
// TODO: This probably should be serde(untagged), but this would be a breaking change
pub enum BucketInlineOrReference {
    Inline(BucketSpec),
    Reference(String),
}

/// Use this struct in your operator.
pub struct ResolvedBucket {
    pub bucket_name: String,
    pub connection: ConnectionSpec,
}

impl BucketInlineOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedBucket, BucketError> {
        match self {
            Self::Inline(inline) => {
                let connection = inline
                    .connection
                    .resolve(client, namespace)
                    .await
                    .context(ResolveConnectionSnafu)?;

                Ok(ResolvedBucket {
                    bucket_name: inline.bucket_name,
                    connection,
                })
            }
            Self::Reference(reference) => {
                let bucket_spec = client
                    .get::<S3Bucket>(&reference, namespace)
                    .await
                    .context(RetrieveS3ConnectionSnafu {
                        s3_connection: reference,
                    })?
                    .spec;

                let connection = bucket_spec
                    .connection
                    .resolve(client, namespace)
                    .await
                    .context(ResolveConnectionSnafu)?;

                Ok(ResolvedBucket {
                    bucket_name: bucket_spec.bucket_name,
                    connection,
                })
            }
        }
    }
}
