mod bucket;
mod connection;

// Publicly re-export unversioned items, in this case errors.
pub use bucket::BucketError;
pub use connection::ConnectionError;

// Group all v1alpha1 items in one module.
pub mod v1alpha1 {
    pub use super::{bucket::v1alpha1::*, connection::v1alpha1::*};
}
