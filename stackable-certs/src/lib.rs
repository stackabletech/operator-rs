//! This crate provides types, traits and functions to work with X.509 TLS
//! certificates. It can be used to create certificate authorities (CAs)
//! which can sign leaf certificates. These leaf certificates can be used
//! for webhook servers or other components which need TLS certificates
//! to encrypt connections.
//!
//! ## Features
//!
//! The crate allows to selectively enable additional features using
//! different feature flags. Currently, these flags are supported:
//!
//! - `k8s`: This enables various traits and functions to work with
//!   certificates and Kubernetes secrets.

use std::path::Path;

#[cfg(feature = "k8s")]
use k8s_openapi::api::core::v1::Secret;

use x509_cert::der::pem::LineEnding;

pub mod ca;
pub mod chain;
pub mod sign;

pub use chain::*;

pub trait CertificateExt: Sized {
    const CERTIFICATE_FILE_EXT: &'static str = "pem";
    const PRIVATE_KEY_FILE_EXT: &'static str = "pk8";

    type Error: std::error::Error;

    /// Reads in a PEM-encoded certificate from `certificate_path` and private
    /// key file from `private_key_path` and finally constructs a CA from the
    /// contents.
    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self, Self::Error>;

    /// Writes the certificate and private key as a PEM-encoded file to
    /// `certificate_path` and `private_key_path` respectively.
    ///
    /// This function will always use [`Self::CERTIFICATE_FILE_EXT`] for the
    /// certificate and [`Self::PRIVATE_KEY_FILE_EXT`] for the private key
    /// file extension.
    fn to_files(
        &self,
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;
}

/// Provides functions to:
///
/// - decode a certificate from a Kubernetes secret
/// - encode a certificate as a Kubernetes secret
#[cfg(feature = "k8s")]
pub trait K8sCertificateExt: Sized {
    type Error: std::error::Error;
    // TODO (@Techassi): Use SecretReference here, for that, we would need to
    // move it out of secret-operator into a common place.
    fn from_secret(secret: Secret) -> Result<Self, Self::Error>;
    fn to_secret(&self) -> Result<Secret, Self::Error>;
}

#[cfg(feature = "k8s")]
pub trait SecretExt {
    type Error: std::error::Error;

    fn requires_renewal(&self) -> bool;
    fn renew(&mut self, renew_after: u64) -> Result<(), Self::Error>;
}
