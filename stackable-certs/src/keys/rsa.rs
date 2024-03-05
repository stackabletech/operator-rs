//! Abstraction layer around the [`rsa`] crate. This module provides types
//! which abstract away the generation of RSA keys used for signing of CAs
//! and other certificates.

use rand_core::{CryptoRngCore, OsRng};
use rsa::{pkcs8::DecodePrivateKey, RsaPrivateKey};
use signature::Keypair;
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::keys::KeypairExt;

pub const DEFAULT_BIT_SIZE: usize = 4096;
pub const MINIMUM_BIT_SIZE: usize = 2048;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create RSA key"))]
    CreateKey { source: rsa::Error },

    #[snafu(display("failed to deserialize the signing (private) key from PEM-encoded PKCS8"))]
    DeserializeSigningKey { source: rsa::pkcs8::Error },

    #[snafu(display("invalid RSA bit size {bit_size}, expected >= {MINIMUM_BIT_SIZE}"))]
    InvalidBitSize { bit_size: usize },
}

#[derive(Debug)]
pub struct SigningKey(rsa::pkcs1v15::SigningKey<sha2::Sha256>);

impl SigningKey {
    // NOTE (@Techassi): Should we maybe enfore bit sizes >= 2048?
    /// Generates a new RSA key with the default random-number generator
    /// [`OsRng`] with the given `bit_size`. Providing [`None`] will use
    /// [`DEFAULT_BIT_SIZE`].
    ///
    /// Common values for `bit_size` are `2048` or `4096`. It should be noted
    /// that the generation of the key takes longer for larger bit sizes. The
    /// generation of an RSA key with a bit size of `4096` can take up to
    /// multiple seconds.
    #[instrument(name = "create_rsa_signing_key")]
    pub fn new(bit_size: Option<usize>) -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with_rng(&mut csprng, bit_size)
    }

    #[instrument(name = "create_rsa_signing_key_custom_rng", skip_all)]
    pub fn new_with_rng<R>(csprng: &mut R, bit_size: Option<usize>) -> Result<Self>
    where
        R: CryptoRngCore + ?Sized,
    {
        let bit_size = bit_size.unwrap_or(DEFAULT_BIT_SIZE);

        if bit_size < MINIMUM_BIT_SIZE {
            return InvalidBitSizeSnafu { bit_size }.fail();
        }

        let private_key = RsaPrivateKey::new(csprng, bit_size).context(CreateKeySnafu)?;
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self(signing_key))
    }
}

impl KeypairExt for SigningKey {
    type SigningKey = rsa::pkcs1v15::SigningKey<sha2::Sha256>;
    type Signature = rsa::pkcs1v15::Signature;
    type VerifyingKey = rsa::pkcs1v15::VerifyingKey<sha2::Sha256>;
    type Error = Error;

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
