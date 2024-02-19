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

use std::path::{Path, PathBuf};

#[cfg(feature = "k8s")]
use {k8s_openapi::api::core::v1::Secret, stackable_operator::client::Client};

use stackable_operator::commons::secret::SecretReference;
use x509_cert::der::pem::LineEnding;

pub mod ca;
pub mod chain;
pub mod sign;

pub use chain::*;

pub const CERTIFICATE_FILE_EXT: &str = "crt";
pub const PRIVATE_KEY_FILE_EXT: &str = "key";

/// This extension trait provides various helper methods to work with TLS
/// certificate related file paths. One use-case is the creation of certificate
/// and private key file paths with the default file extensions.
///
/// ```
/// use std::path::PathBuf;
/// use stackable_certs::PathBufExt;
///
/// let certificate_path = PathBuf::certificate_path("path/to/my/tls-cert");
/// assert_eq!(certificate_path.display().to_string(), "path/to/my/tls-cert.crt");
///
/// let private_key_path = PathBuf::private_key_path("path/to/my/tls-pk");
/// assert_eq!(private_key_path.display().to_string(), "path/to/my/tls-pk.key");
/// ```
pub trait PathBufExt {
    /// Returns a path to `<path>.crt`.
    ///
    /// The default extension is defined by [`CERTIFICATE_FILE_EXT`].
    fn certificate_path(path: impl AsRef<Path>) -> PathBuf {
        PathBuf::from(path.as_ref()).with_extension(CERTIFICATE_FILE_EXT)
    }

    /// Returns a path to `<path>.key`.
    ///
    /// The default extension is defined by [`PRIVATE_KEY_FILE_EXT`].
    fn private_key_path(path: impl AsRef<Path>) -> PathBuf {
        PathBuf::from(path.as_ref()).with_extension(PRIVATE_KEY_FILE_EXT)
    }

    fn certificate_pair_paths(path: impl AsRef<Path>) -> (PathBuf, PathBuf) {
        (
            Self::certificate_path(path.as_ref()),
            Self::private_key_path(path.as_ref()),
        )
    }
}

impl PathBufExt for PathBuf {}

/// This trait provides utilities to work with certificate pairs which contain
/// a public certificate (with a public key embedded in it) and the private key
/// used to sign it. This trait is useful for CAs and self-signed certificates.
pub trait CertificatePair: Sized {
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
    /// It is recommended to use the common file extensions for both the
    /// certificate and private key. These extensions are available as the
    /// contants [`CERTIFICATE_FILE_EXT`] and [`PRIVATE_KEY_FILE_EXT`].
    /// Alternatively, the [`PathBufExt`] trait allows easy creation of correct
    /// paths.
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
pub trait K8sCertificatePair: Sized {
    type Error: std::error::Error;
    // TODO (@Techassi): Use SecretReference here, for that, we would need to
    // move it out of secret-operator into a common place.
    fn from_secret(
        secret: Secret,
        key_certificate: &str,
        key_private_key: &str,
    ) -> Result<Self, Self::Error>;

    #[allow(async_fn_in_trait)]
    async fn from_secret_ref(
        secret_ref: &SecretReference,
        key_certificate: &str,
        key_private_key: &str,
        client: &Client,
    ) -> Result<Self, Self::Error>;
    fn requires_renewal(&self) -> bool;
}
