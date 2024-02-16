use p256::pkcs8::{EncodePublicKey, LineEnding};
use rand_core::{CryptoRngCore, OsRng};
use rsa::{RsaPrivateKey, RsaPublicKey};
use snafu::{ResultExt, Snafu};

pub const DEFAULT_RSA_BIT_SIZE: usize = 2048;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create RSA key"))]
    CreateKey { source: rsa::Error },

    #[snafu(context(false))]
    SerializeKeyToPem { source: x509_cert::spki::Error },
}

#[derive(Debug)]
pub struct SigningKey {
    public_key: RsaPublicKey,
    signing_key: rsa::pkcs1v15::SigningKey<sha2::Sha256>,
}

impl SigningKey {
    pub fn new() -> Result<Self> {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng)
    }

    pub fn new_with<R>(csprng: &mut R) -> Result<Self>
    where
        R: CryptoRngCore + ?Sized,
    {
        let private_key =
            RsaPrivateKey::new(csprng, DEFAULT_RSA_BIT_SIZE).context(CreateKeySnafu)?;
        let public_key = RsaPublicKey::from(&private_key);
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self {
            signing_key,
            public_key,
        })
    }

    pub fn signing_key(&self) -> &rsa::pkcs1v15::SigningKey<sha2::Sha256> {
        &self.signing_key
    }

    pub fn verifying_key(&self) -> &RsaPublicKey {
        &self.public_key
    }

    pub fn verifying_key_pem(&self, line_ending: LineEnding) -> Result<String> {
        Ok(self.verifying_key().to_public_key_pem(line_ending)?)
    }
}
