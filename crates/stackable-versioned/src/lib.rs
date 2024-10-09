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

// Re-export macro
pub use stackable_versioned_macros::*;

#[cfg(feature = "k8s")]
#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("failed to merge CRDs"))]
    MergeCrd { source: kube::core::crd::MergeError },

    #[snafu(display("failed to serialize YAML"))]
    SerializeYaml {
        source: stackable_shared::yaml::Error,
    },
}

// Unused for now, might get picked up again in the future.
#[doc(hidden)]
pub trait AsVersionStr {
    const VERSION: &'static str;

    fn as_version_str(&self) -> &'static str {
        Self::VERSION
    }
}
