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
//! - `webhook`: This enables interoperability between this crates types
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

#[cfg(feature = "webhook")]
use tokio_rustls::rustls::pki_types::CertificateDer;

use p256::pkcs8::EncodePrivateKey;
use snafu::{ResultExt, Snafu};
use stackable_operator::commons::secret::SecretReference;
use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use x509_cert::{
    der::{pem::LineEnding, DecodePem, Encode, EncodePem},
    spki::EncodePublicKey,
    Certificate,
};

pub mod ca;
pub mod keys;

pub const CERTIFICATE_FILE_EXT: &str = "crt";
pub const PRIVATE_KEY_FILE_EXT: &str = "key";

/// Error variants which can be encountered when creating a new
/// [`CertificatePair`].
#[derive(Debug, Snafu)]
pub enum CertificatePairError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("failed to seralize certificate as {key_encoding}"))]
    SerializeCertificate {
        source: x509_cert::der::Error,
        key_encoding: KeyEncoding,
    },

    #[snafu(display("failed to deserialize certificate from {key_encoding}"))]
    DeserializeCertificate {
        source: x509_cert::der::Error,
        key_encoding: KeyEncoding,
    },

    #[snafu(display("failed to serialize private key as PKCS8 {key_encoding}"))]
    SerializePrivateKey {
        source: p256::pkcs8::Error,
        key_encoding: KeyEncoding,
    },

    #[snafu(display("failed to deserialize private key from PKCS8 {key_encoding}"))]
    DeserializePrivateKey {
        source: E,
        key_encoding: KeyEncoding,
    },

    #[snafu(display("failed to write file"))]
    WriteFile { source: std::io::Error },

    #[snafu(display("failed to read file"))]
    ReadFile { source: std::io::Error },
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
    type Error = CertificatePairError<S::Error>;

    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self, Self::Error> {
        let certificate_pem = std::fs::read(certificate_path).context(ReadFileSnafu)?;
        let certificate =
            Certificate::from_pem(&certificate_pem).context(DeserializeCertificateSnafu {
                key_encoding: KeyEncoding::Pem,
            })?;

        let key_pair_pem = std::fs::read_to_string(private_key_path).context(ReadFileSnafu)?;
        let key_pair = S::from_pkcs8_pem(&key_pair_pem).context(DeserializePrivateKeySnafu {
            key_encoding: KeyEncoding::Pem,
        })?;

        Ok(Self {
            certificate,
            key_pair,
        })
    }

    fn to_certificate_file(
        &self,
        certificate_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        let pem = self
            .certificate
            .to_pem(line_ending)
            .context(SerializeCertificateSnafu {
                key_encoding: KeyEncoding::Pem,
            })?;

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
            .context(SerializePrivateKeySnafu {
                key_encoding: KeyEncoding::Pem,
            })?;

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

#[cfg(feature = "webhook")]
impl<S> CertificatePair<S>
where
    S: KeypairExt + 'static,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    pub fn certificate_der(
        &self,
    ) -> Result<CertificateDer<'static>, CertificatePairError<S::Error>> {
        let der = self
            .certificate
            .to_der()
            .context(SerializeCertificateSnafu {
                key_encoding: KeyEncoding::Der,
            })?
            .into();

        Ok(der)
    }

    pub fn private_key_der(
        &self,
    ) -> Result<PrivateKeyDer<'static>, CertificatePairError<S::Error>> {
        // FIXME (@Techassi): Can we make this more elegant?
        let doc = self
            .key_pair
            .signing_key()
            .to_pkcs8_der()
            .context(SerializePrivateKeySnafu {
                key_encoding: KeyEncoding::Der,
            })?;

        let bytes = doc.to_bytes().deref().to_owned();
        let der = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(bytes));

        Ok(der)
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

    /// Save the certificate of the certificate pair as a file at `certificate_path`
    /// with `line_ending`. All implementations of this trait in this crate will use
    /// PEM encoding.
    ///
    /// Use [`LineEnding::default()`] to always use the appropriate line ending
    /// depending on the operating system.
    fn to_certificate_file(
        &self,
        certificate_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;

    /// Save the private key of the certificate pair as a file at `certificate_path`
    /// with `line_ending`.  All implementations of this trait in this crate will use
    /// PEM encoding.
    ///
    /// Use [`LineEnding::default()`] to always use the appropriate line ending
    /// depending on the operating system.
    fn to_private_key_file(
        &self,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;

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

/// Private and public key encoding, either DER or PEM.
#[derive(Debug)]
pub enum KeyEncoding {
    Pem,
    Der,
}

impl std::fmt::Display for KeyEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyEncoding::Pem => write!(f, "PEM"),
            KeyEncoding::Der => write!(f, "DER"),
        }
    }
}
