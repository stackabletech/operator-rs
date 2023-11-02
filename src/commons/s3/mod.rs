mod crd;
mod helpers;

pub use crd::*;
pub use helpers::*;

use snafu::Snafu;
use url::Url;

#[derive(Debug, Snafu)]
pub enum S3Error {
    #[snafu(display("failed to retrieve S3 connection"))]
    RetrieveS3Connection { source: crate::error::Error },

    #[snafu(display("failed to retrieve S3 bucket"))]
    RetrieveS3Bucket { source: crate::error::Error },

    #[snafu(display("failed to parse S3 endpoint"))]
    ParseS3Endpoint { source: url::ParseError },

    #[snafu(display("failed to set S3 endpoint scheme '{scheme}' for endpoint '{endpoint}'"))]
    SetS3EndpointScheme { endpoint: Url, scheme: String },
}

pub type S3Result<T, E = S3Error> = std::result::Result<T, E>;
