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
// #[cfg(feature = "flux-converter")]
// pub mod flux_converter;

#[derive(Debug, Snafu)]
pub enum ParseObjectError {
    #[snafu(display(r#"failed to find "apiVersion" field"#))]
    FieldNotPresent,

    #[snafu(display(r#"the "apiVersion" field is not a string"#))]
    FieldNotStr,

    #[snafu(display("encountered unknown object api version {api_version:?}"))]
    UnknownApiVersion { api_version: String },

    #[snafu(display("failed to deserialize object from json"))]
    Deserialize { source: serde_json::Error },
}

#[derive(Debug, Snafu)]
pub enum ConvertObjectError {
    #[snafu(display("failed to parse object"))]
    Parse { source: ParseObjectError },

    #[snafu(display("failed to serialize object into json"))]
    Serialize { source: serde_json::Error },
}
