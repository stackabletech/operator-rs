//! This crate enables versioning of structs and enums through procedural macros.
//!
//! Currently supported versioning schemes:
//!
//! - Kubernetes API versions (eg: `v1alpha1`, `v1beta1`, `v1`, `v2`), with optional support for
//!   generating CRDs.
//!
//! Support will be extended to SemVer versions, as well as custom version formats in the future.
//!
//! See [`versioned`] for an in-depth usage guide and a list of supported arguments.

// Re-exports
pub use stackable_versioned_macros::versioned;

// Behind k8s feature
#[cfg(feature = "k8s")]
mod k8s;
#[cfg(feature = "k8s")]
pub use k8s::*;

#[cfg(test)]
mod tests;
