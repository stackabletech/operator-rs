//! Abstraction layer around the [`rsa`] crate. This module provides types
//! which abstract away the generation of RSA keys used for signing of CAs
//! and other certificates.
use rand_core::{CryptoRngCore, OsRng};
use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey};
use signature::Keypair;
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::keys::CertificateKeypair;

#[cfg(not(test))]
const KEY_SIZE: usize = 4096;

#[cfg(test)]
const KEY_SIZE: usize = 512;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to create RSA key"))]
    CreateKey { source: rsa::Error },

    #[snafu(display("failed to deserialize the signing (private) key from PEM-encoded PKCS8"))]
    DeserializeSigningKey { source: rsa::pkcs8::Error },
}

#[derive(Debug)]
pub struct SigningKey(rsa::pkcs1v15::SigningKey<sha2::Sha256>);

impl CertificateKeypair for SigningKey {
    type Error = Error;
    type Signature = rsa::pkcs1v15::Signature;
    type SigningKey = rsa::pkcs1v15::SigningKey<sha2::Sha256>;
    type VerifyingKey = rsa::pkcs1v15::VerifyingKey<sha2::Sha256>;

    /// Generates a new RSA key with the default random-number generator
    /// [`OsRng`].
    ///
    /// It should be noted that the generation of the key takes longer for
    /// larger key sizes. The generation of an RSA key with a key size of
    /// `4096` (which is used) can take up to multiple seconds.
    #[instrument(name = "create_rsa_signing_key")]
    fn new() -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with_rng(&mut csprng)
    }

    /// Generates a new RSA key with a custom random-number generator.
    ///
    /// It should be noted that the generation of the key takes longer for
    /// larger key sizes. The generation of an RSA key with a key size of
    /// `4096` (which is used) can take up to multiple seconds.
    #[instrument(name = "create_rsa_signing_key_custom_rng", skip_all)]
    fn new_with_rng<Rng>(rng: &mut Rng) -> Result<Self>
    where
        Rng: CryptoRngCore + ?Sized,
    {
        let private_key = RsaPrivateKey::new(rng, KEY_SIZE).context(CreateKeySnafu)?;
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self(signing_key))
    }

    fn signing_key(&self) -> &Self::SigningKey {
        &self.0
    }

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.0.verifying_key()
    }

    #[instrument(name = "create_rsa_signing_key_from_pkcs8_pem")]
    fn from_pkcs8_pem(input: &str) -> Result<Self, Self::Error> {
        let private_key =
            RsaPrivateKey::from_pkcs8_pem(input).context(DeserializeSigningKeySnafu)?;
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self(signing_key))
    }
}
