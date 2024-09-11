//! This crate enables versioning of structs (and enums in the future). It
//! currently supports Kubernetes API versions while declaring versions on a
//! data type. This will be extended to support SemVer versions, as well as
//! custom version formats in the future.
//!
//! See [`versioned`] for an in-depth usage guide and a list of supported
//! parameters.

pub use stackable_versioned_macros::*;

pub trait AsVersionStr {
    const VERSION: &'static str;

    fn as_version_str(&self) -> &'static str {
        Self::VERSION
    }
}
