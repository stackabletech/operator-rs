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
pub struct SigningKey {
    verifying_key: RsaPublicKey,
    signing_key: rsa::pkcs1v15::SigningKey<sha2::Sha256>,
}

impl SigningKey {
    #[instrument(name = "create_rsa_signing_key")]
    pub fn new() -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng)
    }

    #[instrument(name = "create_rsa_signing_key_custom_rng", skip_all)]
    pub fn new_with<R>(csprng: &mut R) -> Result<Self>
    where
        R: CryptoRngCore + ?Sized,
    {
        let private_key =
            RsaPrivateKey::new(csprng, DEFAULT_RSA_BIT_SIZE).context(CreateKeySnafu)?;
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
