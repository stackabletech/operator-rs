mod bucket;
mod connection;

pub use bucket::S3Bucket;
pub use connection::S3Connection;

// Group all v1alpha1 items in one module.
pub mod v1alpha1 {
    pub use super::{bucket::v1alpha1::*, connection::v1alpha1::*};
}
