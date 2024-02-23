//! This crate provides types, traits and functions to work with X.509 TLS
//! certificates. It can be used to create certificate authorities (CAs)
//! which can sign leaf certificates. These leaf certificates can be used
//! for webhook servers or other components which need TLS certificates
//! to encrypt connections.
//!
//! ## Feature Flags
//!
//! The crate allows to selectively enable additional features using
//! different feature flags. Currently, these flags are supported:
//!
//! - `k8s`: This enables various traits and functions to work with
//!   certificates and Kubernetes secrets.
//! - `rustls`: This enables interoperability between this crates types
//!   and the certificate formats required for the `stackable-webhook`
//!   crate.
//!
//! ## References
//!
//! - <https://cabforum.org/uploads/CA-Browser-Forum-TLS-BRs-v2.0.2.pdf>
//! - <https://datatracker.ietf.org/doc/html/rfc5280>
//! - <https://github.com/zmap/zlint>

use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

use crate::keys::KeypairExt;

#[cfg(feature = "k8s")]
use {k8s_openapi::api::core::v1::Secret, stackable_operator::client::Client};

#[cfg(feature = "rustls")]
use tokio_rustls::rustls::pki_types::CertificateDer;

use p256::pkcs8::EncodePrivateKey;
use snafu::{ResultExt, Snafu};
use stackable_operator::commons::secret::SecretReference;
use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use x509_cert::{
    der::{pem::LineEnding, Encode, EncodePem},
    spki::EncodePublicKey,
    Certificate,
};

pub mod ca;
pub mod keys;

pub const CERTIFICATE_FILE_EXT: &str = "crt";
pub const PRIVATE_KEY_FILE_EXT: &str = "key";

#[derive(Debug, Snafu)]
pub enum CertificatePairError {
    #[snafu(display("failed to seralize certificate as PEM"))]
    SerializeCertificate { source: x509_cert::der::Error },

    #[snafu(display("failed to seralize private key as PKCS8 PEM"))]
    SerializePrivateKey { source: p256::pkcs8::Error },

    #[snafu(display("failed to write file"))]
    WriteFile { source: std::io::Error },
}

/// Contains the certificate and the signing / embedded key pair.
///
/// A [`CertificateAuthority`](crate::ca::CertificateAuthority) uses this struct
/// internally to store the signing key pair which is used to sign the CA
/// itself (self-signed) and all child leaf certificates. Leaf certificates on
/// the other hand use this to store the bound keypair.
#[derive(Debug)]
pub struct CertificatePair<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate: Certificate,
    key_pair: S,
}

impl<S> CertificatePairExt for CertificatePair<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    type Error = CertificatePairError;

    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self, Self::Error> {
        todo!()
    }

    fn to_certificate_file(
        &self,
        certificate_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        let pem = self
            .certificate
            .to_pem(line_ending)
            .context(SerializeCertificateSnafu)?;

        std::fs::write(certificate_path, pem).context(WriteFileSnafu)
    }

    fn to_private_key_file(
        &self,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        let pem = self
            .key_pair
            .signing_key()
            .to_pkcs8_pem(line_ending)
            .context(SerializePrivateKeySnafu)?;

        std::fs::write(private_key_path, pem).context(WriteFileSnafu)
    }
}

impl<S> CertificatePair<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Returns a reference to the [`Certificate`].
    pub fn certificate(&self) -> &Certificate {
        &self.certificate
    }

    /// Returns a reference to the (signing) key pair.
    pub fn key_pair(&self) -> &S {
        &self.key_pair
    }
}

#[cfg(feature = "rustls")]
impl<S> CertificatePair<S>
where
    S: KeypairExt + 'static,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    pub fn certificate_der(&self) -> CertificateDer<'static> {
        // TODO (@Techassi): Remove unwrap
        self.certificate.to_der().unwrap().into()
    }

    pub fn private_key_der(&self) -> PrivateKeyDer<'static> {
        // TODO (@Techassi): Remove unwrap
        // FIXME (@Techassi): Can we make this any more elegant?
        let bytes = self
            .key_pair
            .signing_key()
            .to_pkcs8_der()
            .unwrap()
            .to_bytes()
            .deref()
            .to_owned();

        PrivateKeyDer::from(PrivatePkcs8KeyDer::from(bytes))
    }
}

/// Provides utilities to work with certificate pairs which contain a public
/// certificate (with a public key embedded in it) and the private key used to
/// sign it. This trait is useful for CAs and self-signed certificates.
pub trait CertificatePairExt: Sized {
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
    ) -> Result<(), Self::Error> {
        self.to_certificate_file(certificate_path, line_ending)?;
        self.to_private_key_file(private_key_path, line_ending)
    }

    fn to_certificate_file(
        &self,
        certificate_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;

    fn to_private_key_file(
        &self,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;
}

/// Provides functions to work with CAs stored in Kubernetes secrets.
///
/// Namely these function enable:
///
/// - decoding a certificate from a Kubernetes secret
/// - encoding a certificate as a Kubernetes secret
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

/// Provides various helper methods to work with TLS certificate related file
/// paths.
///
/// One use-case is the creation of certificate and private key file paths with
/// the default file extensions.
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

/// Supported private key types, currently [RSA](crate::keys::rsa) and
/// [ECDSA](crate::keys::ecdsa).
#[derive(Debug)]
pub enum PrivateKeyType {
    Ecdsa,
    Rsa,
}
