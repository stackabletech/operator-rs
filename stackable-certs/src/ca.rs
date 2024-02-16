use std::{path::Path, str::FromStr, time::Duration};

#[cfg(feature = "k8s")]
use k8s_openapi::api::core::v1::Secret;

use p256::pkcs8::EncodePrivateKey;
use snafu::Snafu;
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, DecodePem, EncodePem},
    ext::pkix::BasicConstraints,
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
    Certificate,
};

#[cfg(feature = "k8s")]
use crate::K8sCertificateExt;

use crate::{sign::rsa::SigningKey, CertificateExt};

// NOTE (@Techassi): Not all this code should live here, there will be other
// modules which handle a subset of features. For now this mostly serves as
// a rough scratch pad to sketch out the general pieces of code and get more
// comfortable with the x509_cert crate.

/// The root CA subject name containing the common name, organization name and
/// country.
pub const ROOT_CA_SUBJECT: &str = "CN=Stackable Root CA,O=Stackable GmbH,C=DE";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
// TODO (@Techassi): Add a CA builder

/// A certificate authority (CA) which is used to generate and sign
/// intermediate certificates.
#[derive(Debug)]
pub struct CertificateAuthority {
    serial_numbers: Vec<u64>,
    certificate: Certificate,
    signing_key: SigningKey,
}

impl CertificateExt for CertificateAuthority {
    type Error = Error;

    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self> {
        todo!()
    }

    fn to_files(
        &self,
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<()> {
        let certificate_path = certificate_path
            .as_ref()
            .with_extension(Self::CERTIFICATE_FILE_EXT);
        let certificate_pem = self.certificate.to_pem(line_ending).unwrap();
        std::fs::write(certificate_path, certificate_pem).unwrap();

        let private_key_path = private_key_path
            .as_ref()
            .with_extension(Self::PRIVATE_KEY_FILE_EXT);
        let private_key_pem = self
            .signing_key
            .signing_key()
            .to_pkcs8_pem(line_ending)
            .unwrap();
        std::fs::write(private_key_path, private_key_pem).unwrap();

        Ok(())
    }
}

#[cfg(feature = "k8s")]
impl K8sCertificateExt for CertificateAuthority {
    type Error = Error;

    fn from_secret(secret: Secret) -> crate::Result<Self, Self::Error> {
        todo!()
    }

    fn to_secret(&self) -> crate::Result<Secret, Self::Error> {
        todo!()
    }
}

impl CertificateAuthority {
    /// Creates a new CA certificate which embeds the public part of the randomly
    /// generated signing key. The certificate is additionally signed by the
    /// private part of the signing key.
    pub fn new() -> Self {
        let signing_key = SigningKey::new().unwrap();
        let serial_number = SerialNumber::from(rand::random::<u64>());
        let validity = Validity::from_now(Duration::from_secs(3600)).unwrap();
        let subject = Name::from_str(ROOT_CA_SUBJECT).unwrap();
        let spki = SubjectPublicKeyInfoOwned::from_pem(
            signing_key
                .verifying_key_pem(LineEnding::default())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

        let signer = signing_key.signing_key();

        let mut builder = CertificateBuilder::new(
            Profile::Root,
            serial_number,
            validity,
            subject,
            spki,
            signer,
        )
        .unwrap();

        builder
            .add_extension(&BasicConstraints {
                ca: true,
                path_len_constraint: None,
            })
            .unwrap();

        let certificate = builder.build().unwrap();

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let ca = CertificateAuthority::new();
        ca.to_files("cert", "key", LineEnding::default()).unwrap();
    }
}
