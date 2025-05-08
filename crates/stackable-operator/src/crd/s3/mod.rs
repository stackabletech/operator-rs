mod bucket;
mod connection;

// Group all v1alpha1 items in one module.
pub mod v1alpha1 {
    pub use super::{bucket::v1alpha1::*, connection::v1alpha1::*};
}
