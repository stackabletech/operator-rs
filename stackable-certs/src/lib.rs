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

use std::ops::Deref;

use crate::keys::KeypairExt;

#[cfg(feature = "k8s")]
use {k8s_openapi::api::core::v1::Secret, stackable_operator::client::Client};

#[cfg(feature = "webhook")]
use tokio_rustls::rustls::pki_types::CertificateDer;

use p256::pkcs8::EncodePrivateKey;
use snafu::{ResultExt, Snafu};
use stackable_operator::commons::secret::SecretReference;
use tokio_rustls::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use x509_cert::{der::Encode, spki::EncodePublicKey, Certificate};

pub mod ca;
pub mod keys;

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

/// Provides functions to work with CAs stored in Kubernetes secrets.
///
/// Namely these function enable:
///
/// - decoding a certificate from a Kubernetes secret
/// - encoding a certificate as a Kubernetes secret
#[cfg(feature = "k8s")]
pub trait CertificatePairExt: Sized {
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
