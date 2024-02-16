use std::ops::Deref;

use p256::pkcs8::{EncodePublicKey, LineEnding};
use rand_core::{CryptoRngCore, OsRng};
use snafu::Snafu;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false))]
    SerializeKeyToPem { source: x509_cert::spki::Error },
}

pub struct SigningKey(p256::ecdsa::SigningKey);

impl Deref for SigningKey {
    type Target = p256::ecdsa::SigningKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
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
        Ok(Self(signing_key))
    }

    pub fn signing_key(&self) -> &Self {
        self
    }

    pub fn verifying_key(&self) -> &p256::ecdsa::VerifyingKey {
        todo!()
    }

    pub fn verifying_key_pem(&self, line_ending: LineEnding) -> Result<String> {
        Ok(self.verifying_key().to_public_key_pem(line_ending)?)
    }
}
