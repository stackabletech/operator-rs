use std::{path::Path, str::FromStr, time::Duration};

#[cfg(feature = "k8s")]
use {
    k8s_openapi::api::core::v1::Secret,
    kube::ResourceExt,
    stackable_operator::{client::Client, commons::secret::SecretReference},
};

use snafu::{OptionExt, ResultExt, Snafu};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, DecodePem, EncodePem},
    ext::pkix::{BasicConstraints, KeyUsage, KeyUsages},
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
    Certificate,
};

use crate::{sign::rsa::SigningKey, CertificatePair, K8sCertificatePair};

/// The root CA subject name containing the common name, organization name and
/// country.
pub const ROOT_CA_SUBJECT: &str = "CN=Stackable Root CA,O=Stackable GmbH,C=DE";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {}

#[derive(Debug, Snafu)]
pub enum SecretError {
    #[snafu(display("the secret {secret:?} does not contain any data"))]
    NoSecretData { secret: String },

    #[snafu(display("the secret {secret:?} does not contain TLS certificate data"))]
    NoCertificateData { secret: String },

    #[snafu(display("failed to read PEM-encoded certificate chain from secret {secret:?}"))]
    ReadChain {
        source: x509_cert::der::Error,
        secret: String,
    },

    #[snafu(display("the secret {secret:?} does not contain TLS private key data"))]
    NoPrivateKeyData { secret: String },

    #[snafu(display("failed to read PEM-encoded signing (private) key from secret {secret:?}"))]
    ReadSigningKey {
        source: crate::sign::rsa::Error,
        secret: String,
    },
}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
// TODO (@Techassi): Add a CA builder

/// A certificate authority (CA) which is used to generate and sign
/// intermidiate or leaf certificates.
#[derive(Debug)]
pub struct CertificateAuthority {
    serial_numbers: Vec<u64>,
    certificate: Certificate,
    signing_key: SigningKey,
}

impl CertificatePair for CertificateAuthority {
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
        let certificate_pem = self.certificate.to_pem(line_ending).unwrap();
        std::fs::write(certificate_path, certificate_pem).unwrap();

        let private_key_pem = self.signing_key.signing_key_pem(line_ending).unwrap();
        std::fs::write(private_key_path, private_key_pem).unwrap();

        Ok(())
    }
}

#[cfg(feature = "k8s")]
impl K8sCertificatePair for CertificateAuthority {
    type Error = SecretError;

    fn from_secret(
        secret: Secret,
        key_certificate: &str,
        key_private_key: &str,
    ) -> Result<Self, Self::Error> {
        let name = secret.name_any();
        let data = secret.data.context(NoSecretDataSnafu {
            secret: name.clone(),
        })?;

        let certificate_data = data.get(key_certificate).context(NoCertificateDataSnafu {
            secret: name.clone(),
        })?;

        let certificate = Certificate::load_pem_chain(&certificate_data.0)
            .context(ReadChainSnafu {
                secret: name.clone(),
            })?
            .remove(0);

        let private_key_data = data.get(key_private_key).context(NoPrivateKeyDataSnafu {
            secret: name.clone(),
        })?;

        // TODO (@Techassi): Remove unwrap
        let signing_key =
            SigningKey::from_pkcs8_pem(std::str::from_utf8(&private_key_data.0).unwrap())
                .context(ReadSigningKeySnafu { secret: name })?;

        Ok(Self {
            serial_numbers: Vec::new(),
            certificate,
            signing_key,
        })
    }

    async fn from_secret_ref(
        secret_ref: &SecretReference,
        key_certificate: &str,
        key_private_key: &str,
        client: &Client,
    ) -> Result<Self, Self::Error> {
        // TODO (@Techassi): Remove unwrap
        let secret_api = client.get_api::<Secret>(&secret_ref.namespace);
        let secret = secret_api.get(&secret_ref.name).await.unwrap();
        Self::from_secret(secret, key_certificate, key_private_key)
    }

    fn requires_renewal(&self) -> bool {
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

        // NOTE (@Techassi): Do we need the SubjectKeyIdentifier extension? If
        // so, with which value. Also, what about the AuthorityKeyIdentifier?
        builder
            .add_extension(&BasicConstraints {
                ca: true,
                path_len_constraint: None,
            })
            .unwrap();

        builder
            .add_extension(&KeyUsage(
                KeyUsages::DigitalSignature | KeyUsages::KeyCertSign | KeyUsages::CRLSign,
            ))
            .unwrap();

        let certificate = builder.build().unwrap();

        Self {
            serial_numbers: Vec::new(),
            certificate,
            signing_key,
        }
    }

    pub fn generate_leaf_certificate() {}
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::PathBufExt;

    use super::*;

    #[test]
    fn test() {
        let ca = CertificateAuthority::new();
        let (cert_path, pk_path) = PathBuf::certificate_pair_paths("tls");
        ca.to_files(cert_path, pk_path, LineEnding::default())
            .unwrap();
    }
}
