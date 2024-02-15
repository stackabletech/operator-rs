use std::{str::FromStr, time::Duration};

use ed25519_dalek::pkcs8::EncodePublicKey;
use snafu::Snafu;
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, DecodePem},
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
    Certificate,
};

use crate::sign::{RsaSigningKey, Signer};

// NOTE (@Techassi): Not all this code should live here, there will be other
// modules which handle a subset of features. For now this mostly serves as
// a rough scratch pad to sketch out the general pieces of code and get more
// comfortable with the x509_cert crate.

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
// TODO (@Techassi): Add a CA builder

/// A certificate authority (CA) which is used to generate and sign
/// intermediate certificates.
#[derive(Debug)]
pub struct CertificateAuthority<S> {
    serial_numbers: Vec<u64>,
    certificate: Certificate,
    signing_key: S,
}

impl CertificateAuthority<RsaSigningKey> {
    pub fn new() -> Self {
        let signing_key = RsaSigningKey::new();
        dbg!(&signing_key);

        let serial_number = SerialNumber::from(rand::random::<u64>());
        let validity = Validity::from_now(Duration::from_secs(3600)).unwrap();
        let subject = Name::from_str("CN=Stackable Root CA,O=Stackable GmbH,C=DE").unwrap();
        let spki = SubjectPublicKeyInfoOwned::from_pem(
            signing_key
                .public_key
                .to_public_key_pem(LineEnding::default())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

        let signer = signing_key.signing_key();

        let certificate = CertificateBuilder::new(
            Profile::Root,
            serial_number,
            validity,
            subject,
            spki,
            &signer,
        )
        .unwrap()
        .build()
        .unwrap();

        Self {
            serial_numbers: Vec::new(),
            certificate,
            signing_key,
        }
    }

    pub fn generate_intermediate_certificate(&self) -> Result<IntermediateCertificate> {
        todo!()
    }
}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
#[derive(Debug)]
pub struct IntermediateCertificate {
    certificate: String,
    private_key: String,
    validity: Validity,
}

impl IntermediateCertificate {
    pub fn generate_leaf_certificate(&self) -> Result<Certificate> {
        todo!()
    }
}

// TODO (@Techassi): Maybe add functions to read/write certificates from/to K8s
// secrets or separate those out into a K8s specific trait.
pub trait CertificateExt: Sized {
    fn from_file() -> Result<Self>;
    fn into_file(self) -> Result<()>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let ca = CertificateAuthority::new();
    }
}
