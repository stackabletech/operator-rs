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
//! - `rustls`: This enables interoperability between this crates types
//!   and the certificate formats required for the `stackable-webhook`
//!   crate.
//!
//! ## References
//!
//! - <https://cabforum.org/uploads/CA-Browser-Forum-TLS-BRs-v2.0.2.pdf>
//! - <https://datatracker.ietf.org/doc/html/rfc5280>
//! - <https://github.com/zmap/zlint>
#[cfg(feature = "rustls")]
use std::ops::Deref;

#[cfg(feature = "rustls")]
use {
    p256::pkcs8::EncodePrivateKey,
    snafu::ResultExt,
    tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    x509_cert::der::Encode,
};

use snafu::Snafu;
use x509_cert::{spki::EncodePublicKey, Certificate};

use crate::keys::CertificateKeypair;

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

/// Custom implementation of [`std::cmp::PartialEq`] because [`std::io::Error`] doesn't implement it, but [`std::io::ErrorKind`] does.
impl<E: snafu::Error + std::cmp::PartialEq> PartialEq for CertificatePairError<E> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::WriteFile { source: lhs_source }, Self::WriteFile { source: rhs_source }) => {
                lhs_source.kind() == rhs_source.kind()
            }
            (Self::ReadFile { source: lhs_source }, Self::ReadFile { source: rhs_source }) => {
                lhs_source.kind() == rhs_source.kind()
            }
            (lhs, rhs) => lhs == rhs,
        }
    }
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
    S: CertificateKeypair,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate: Certificate,
    key_pair: S,
}

impl<S> CertificatePair<S>
where
    S: CertificateKeypair,
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
    S: CertificateKeypair + 'static,
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

/// Supported private key types, currently [RSA](crate::keys::rsa) and
/// [ECDSA](crate::keys::ecdsa).
#[derive(Debug)]
pub enum PrivateKeyType {
    Ecdsa,
    Rsa,
}

/// Private and public key encoding, either DER or PEM.
#[derive(Debug, PartialEq)]
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
