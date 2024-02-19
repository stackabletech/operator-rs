use p256::pkcs8::{EncodePublicKey, LineEnding};
use rand_core::{CryptoRngCore, OsRng};
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
    RsaPrivateKey, RsaPublicKey,
};
use snafu::{ResultExt, Snafu};
use zeroize::Zeroizing;

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

    pub fn from_pkcs8_pem(input: &str) -> Result<Self> {
        let private_key =
            RsaPrivateKey::from_pkcs8_pem(input).context(DeserializeSigningKeySnafu)?;
        let public_key = RsaPublicKey::from(&private_key);
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);

        Ok(Self {
            public_key,
            signing_key,
        })
    }

    pub fn signing_key(&self) -> &rsa::pkcs1v15::SigningKey<sha2::Sha256> {
        &self.signing_key
    }

    pub fn signing_key_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        self.signing_key()
            .to_pkcs8_pem(line_ending)
            .context(SerializeSigningKeyToPemSnafu)
    }

    pub fn verifying_key(&self) -> &RsaPublicKey {
        &self.public_key
    }

    pub fn verifying_key_pem(&self, line_ending: LineEnding) -> Result<String> {
        self.verifying_key()
            .to_public_key_pem(line_ending)
            .context(SerializeVerifyingKeyToPemSnafu)
    }
}
