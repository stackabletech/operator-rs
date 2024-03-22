//! Abstraction layer around the [`ecdsa`] crate. This module provides types
//! which abstract away the generation of ECDSA keys used for signing of CAs
//! and other certificates.
use p256::{pkcs8::DecodePrivateKey, NistP256};
use rand_core::{CryptoRngCore, OsRng};
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::keys::CertificateKeypair;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false))]
    SerializeKeyToPem { source: x509_cert::spki::Error },

    #[snafu(display("failed to deserialize ECDSA key from PEM"))]
    DeserializeKeyFromPem { source: p256::pkcs8::Error },
}

#[derive(Debug)]
pub struct SigningKey(p256::ecdsa::SigningKey);

impl SigningKey {
    #[instrument(name = "create_ecdsa_signing_key")]
    pub fn new() -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with_rng(&mut csprng)
    }

    #[instrument(name = "create_ecdsa_signing_key_custom_rng", skip_all)]
    pub fn new_with_rng<R>(csprng: &mut R) -> Result<Self>
    where
        R: CryptoRngCore + Sized,
    {
        let signing_key = p256::ecdsa::SigningKey::random(csprng);
        Ok(Self(signing_key))
    }
}

impl CertificateKeypair for SigningKey {
    type SigningKey = p256::ecdsa::SigningKey;
    type Signature = ecdsa::der::Signature<NistP256>;
    type VerifyingKey = p256::ecdsa::VerifyingKey;

    type Error = Error;

    fn signing_key(&self) -> &Self::SigningKey {
        &self.0
    }

    fn verifying_key(&self) -> Self::VerifyingKey {
        *self.0.verifying_key()
    }

    #[instrument(name = "create_ecdsa_signing_key_from_pkcs8_pem")]
    fn from_pkcs8_pem(input: &str) -> Result<Self, Self::Error> {
        let signing_key =
            p256::ecdsa::SigningKey::from_pkcs8_pem(input).context(DeserializeKeyFromPemSnafu)?;

        Ok(Self(signing_key))
    }
}
