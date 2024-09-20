mod crd;
mod helpers;

pub use crd::*;
pub use helpers::*;

use snafu::Snafu;
use url::Url;

use crate::builder::{self};

use super::{secret_class::SecretClassVolumeError, tls_verification::TlsClientDetailsError};

#[derive(Debug, Snafu)]
pub enum S3Error {
    #[snafu(display("failed to retrieve S3 connection '{s3_connection}'"))]
    RetrieveS3Connection {
        source: crate::client::Error,
        s3_connection: String,
    },

    #[snafu(display("failed to parse S3 endpoint '{endpoint}'"))]
    ParseS3Endpoint {
        source: url::ParseError,
        endpoint: String,
    },

    #[snafu(display("failed to set S3 endpoint scheme '{scheme}' for endpoint '{endpoint}'"))]
    SetS3EndpointScheme { endpoint: Url, scheme: String },

    #[snafu(display("failed to add S3 credential volumes and volume mounts"))]
    AddS3CredentialVolumes { source: SecretClassVolumeError },

    #[snafu(display("failed to add S3 TLS client details volumes and volume mounts"))]
    AddS3TlsClientDetailsVolumes { source: TlsClientDetailsError },

    #[snafu(display("failed to add needed volumes"))]
    AddVolumes { source: builder::pod::Error },

    #[snafu(display("failed to add needed volumeMounts"))]
    AddVolumeMounts {
        source: builder::pod::container::Error,
    },
}
