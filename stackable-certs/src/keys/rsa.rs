//! Abstraction layer around the [`rsa`] crate. This module provides types
//! which abstract away the generation of RSA keys used for signing of CAs
//! and other certificates.
use rand_core::{CryptoRngCore, OsRng};
use rsa::{pkcs8::DecodePrivateKey, RsaPrivateKey};
use signature::Keypair;
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::keys::CertificateKeypair;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create RSA key"))]
    CreateKey { source: rsa::Error },

    #[snafu(display("failed to deserialize the signing (private) key from PEM-encoded PKCS8"))]
    DeserializeSigningKey { source: rsa::pkcs8::Error },
}

#[derive(Debug)]
pub struct SigningKey(rsa::pkcs1v15::SigningKey<sha2::Sha256>);

impl SigningKey {
    /// Generates a new RSA key with the default random-number generator
    /// [`OsRng`] with the given `key_size`.
    ///
    /// Common values for `key_size` are `2048` or `4096`. It should be noted
    /// that the generation of the key takes longer for larger key sizes. The
    /// generation of an RSA key with a key size of `4096` can take up to
    /// multiple seconds.
    #[instrument(name = "create_rsa_signing_key")]
    pub fn new(key_size: KeySize) -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with_rng(&mut csprng, key_size)
    }

    #[instrument(name = "create_rsa_signing_key_custom_rng", skip_all)]
    pub fn new_with_rng<R>(csprng: &mut R, key_size: KeySize) -> Result<Self>
    where
        R: CryptoRngCore + ?Sized,
    {
        let private_key = RsaPrivateKey::new(csprng, key_size.bits()).context(CreateKeySnafu)?;
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self(signing_key))
    }
}

impl CertificateKeypair for SigningKey {
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

/// The key size of an RSA key pair.
///
/// This can either be:
///
/// - [`KeySize::Default`], with a value of `4096`
/// - [`KeySize::Minimum`], with a value of `2048`
#[derive(Debug, Default, Clone, Copy)]
pub enum KeySize {
    #[default]
    Default,
    Minimum,
}

impl KeySize {
    pub fn bits(self) -> usize {
        match self {
            KeySize::Default => 2048,
            KeySize::Minimum => 4096,
        }
    }
}
