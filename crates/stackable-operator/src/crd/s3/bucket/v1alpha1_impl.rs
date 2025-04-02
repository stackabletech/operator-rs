//! v1alpha1 specific implementations for S3 buckets.

use snafu::ResultExt as _;

use crate::{
    client::Client,
    crd::s3::bucket::{
        BucketError, ResolveConnectionSnafu, RetrieveS3ConnectionSnafu,
        v1alpha1::{InlineBucketOrReference, ResolvedBucket, S3Bucket},
    },
};

impl InlineBucketOrReference {
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
