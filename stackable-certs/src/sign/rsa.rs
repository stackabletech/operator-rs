//! Abstraction layer between this crate and the [`rsa`] crate. This module
//! provides types which abstract away the generation of RSA keys used for
//! signing of CAs and other certificates.

use rand_core::{CryptoRngCore, OsRng};
use rsa::{pkcs8::DecodePrivateKey, RsaPrivateKey, RsaPublicKey};
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::sign::SigningKeyPair;

pub const DEFAULT_RSA_BIT_SIZE: usize = 2048;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create RSA key"))]
    CreateKey { source: rsa::Error },

    #[snafu(display("failed to deserialize the signing (private) key from PEM-encoded PKCS8"))]
    DeserializeSigningKey { source: rsa::pkcs8::Error },

    #[snafu(display("failed to serialize the signing (private) key as PEM-encoded PKCS8"))]
    SerializeSigningKeyToPem { source: rsa::pkcs8::Error },

    #[snafu(display("failed to serialize the verifying (public) key as PEM-encoded SPKI"))]
    SerializeVerifyingKeyToPem { source: x509_cert::spki::Error },
}

#[derive(Debug)]
pub struct Options {
    pub bit_size: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            bit_size: DEFAULT_RSA_BIT_SIZE,
        }
    }
}

#[derive(Debug)]
pub struct SigningKey {
    verifying_key: RsaPublicKey,
    signing_key: rsa::pkcs1v15::SigningKey<sha2::Sha256>,
}

impl SigningKey {
    // NOTE (@Techassi): Should we maybe enfore bit sizes >= 2048?
    /// Generates a new RSA key with the default random-number generator
    /// [`OsRng`] with the given `bit_size`. Providing [`None`] will use
    /// [`DEFAULT_RSA_BIT_SIZE`].
    ///
    /// Common values for `bit_size` are `2048` or `4096`. It should be noted
    /// that the generation of the key takes longer for larger bit sizes. The
    /// generation of an RSA key with a bit size of `4096` can take up to
    /// multiple seconds.
    #[instrument(name = "create_rsa_signing_key")]
    pub fn new(bit_size: Option<usize>) -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng, bit_size)
    }

    #[instrument(name = "create_rsa_signing_key_custom_rng", skip_all)]
    pub fn new_with<R>(csprng: &mut R, bit_size: Option<usize>) -> Result<Self>
    where
        R: CryptoRngCore + ?Sized,
    {
        let private_key = RsaPrivateKey::new(csprng, bit_size.unwrap_or(DEFAULT_RSA_BIT_SIZE))
            .context(CreateKeySnafu)?;
        let verifying_key = RsaPublicKey::from(&private_key);
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }
}

impl SigningKeyPair for SigningKey {
    type SigningKey = rsa::pkcs1v15::SigningKey<sha2::Sha256>;
    type Signature = rsa::pkcs1v15::Signature;
    type VerifyingKey = rsa::RsaPublicKey;
    type Error = Error;

    fn private_key(&self) -> &Self::SigningKey {
        &self.signing_key
    }

    fn public_key(&self) -> &Self::VerifyingKey {
        &self.verifying_key
    }

    #[instrument(name = "create_rsa_signing_key_from_pkcs8_pem")]
    fn from_pkcs8_pem(input: &str) -> Result<Self, Self::Error> {
        let private_key =
            RsaPrivateKey::from_pkcs8_pem(input).context(DeserializeSigningKeySnafu)?;
        let verifying_key = RsaPublicKey::from(&private_key);
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self {
            verifying_key,
            signing_key,
        })
    }
}
