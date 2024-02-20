use p256::{pkcs8::DecodePrivateKey, NistP256};
use rand_core::{CryptoRngCore, OsRng};
use snafu::Snafu;

use crate::sign::SigningKeyPair;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false))]
    SerializeKeyToPem { source: x509_cert::spki::Error },
}

#[derive(Debug)]
pub struct SigningKey {
    verifying_key: p256::ecdsa::VerifyingKey,
    signing_key: p256::ecdsa::SigningKey,
}

impl SigningKey {
    pub fn new() -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng)
    }

    pub fn new_with<R>(csprng: &mut R) -> Result<Self>
    where
        R: CryptoRngCore + Sized,
    {
        let signing_key = p256::ecdsa::SigningKey::random(csprng);
        let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }
}

impl SigningKeyPair for SigningKey {
    type SigningKey = p256::ecdsa::SigningKey;
    type Signature = ecdsa::der::Signature<NistP256>;
    type VerifyingKey = p256::ecdsa::VerifyingKey;

    type Error = Error;

    fn private_key(&self) -> &Self::SigningKey {
        &self.signing_key
    }

    fn public_key(&self) -> &Self::VerifyingKey {
        &self.verifying_key
    }

    fn from_pkcs8_pem(input: &str) -> Result<Self, Self::Error> {
        // TODO (@Techassi): Remove unwrap
        let signing_key = p256::ecdsa::SigningKey::from_pkcs8_pem(input).unwrap();
        let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);

        Ok(Self {
            verifying_key,
            signing_key,
        })
    }
}
