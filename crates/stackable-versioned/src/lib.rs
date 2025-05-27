//! This crate enables versioning of structs and enums through procedural
//! macros.
//!
//! Currently supported versioning schemes:
//!
//! - Kubernetes API versions (eg: `v1alpha1`, `v1beta1`, `v1`, `v2`), with
//!   optional support for generating CRDs.
//!
//! Support will be extended to SemVer versions, as well as custom version
//! formats in the future.
//!
//! See [`versioned`] for an in-depth usage guide and a list of supported
//! parameters.
use snafu::Snafu;
// Re-exports
pub use stackable_versioned_macros::versioned;

// Behind k8s feature
#[cfg(feature = "k8s")]
mod k8s;
#[cfg(feature = "k8s")]
pub use k8s::*;

// Behind flux-converter feature
#[cfg(feature = "flux-converter")]
mod flux_converter;
#[cfg(feature = "flux-converter")]
pub use flux_converter::*;

#[derive(Debug, Snafu)]
pub enum ParseResourceVersionError {
    #[snafu(display("the resource version \"{version}\" is not known"))]
    UnknownResourceVersion { version: String },

    #[snafu(display("the api version \"{api_version}\" is not known"))]
    UnknownApiVersion { api_version: String },
}
