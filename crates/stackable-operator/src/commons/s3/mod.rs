mod crd;
mod helpers;

pub use crd::*;
pub use helpers::*;

use snafu::Snafu;
use url::Url;

use super::{secret_class::SecretClassVolumeError, tls_verification::TlsClientDetailsError};

#[derive(Debug, Snafu)]
pub enum S3Error {
    #[snafu(display("failed to retrieve S3 connection"))]
    RetrieveS3Connection { source: crate::client::Error },

    #[snafu(display("failed to retrieve S3 bucket"))]
    RetrieveS3Bucket { source: crate::client::Error },

    #[snafu(display("failed to parse S3 endpoint"))]
    ParseS3Endpoint { source: url::ParseError },

    #[snafu(display("failed to set S3 endpoint scheme '{scheme}' for endpoint '{endpoint}'"))]
    SetS3EndpointScheme { endpoint: Url, scheme: String },

    #[snafu(display("failed to add S3 credential volumes and volume mounts"))]
    AddS3CredentialVolumes { source: SecretClassVolumeError },

    #[snafu(display("failed to add S3 TLS client details volumes and volume mounts"))]
    AddS3TlsClientDetailsVolumes { source: TlsClientDetailsError },
}
