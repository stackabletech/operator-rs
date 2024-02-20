//! Contains types and functions to generate and sign certificate authorities
//! (CAs).
use std::{path::Path, str::FromStr, time::Duration};

#[cfg(feature = "k8s")]
use {
    crate::K8sCertificatePair,
    k8s_openapi::api::core::v1::Secret,
    kube::ResourceExt,
    stackable_operator::{client::Client, commons::secret::SecretReference},
};

use p256::pkcs8::EncodePrivateKey;
use signature::Keypair;
use snafu::{OptionExt, ResultExt, Snafu};
use tracing::{info, instrument};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, referenced::OwnedToRef, DecodePem, EncodePem},
    ext::pkix::AuthorityKeyIdentifier,
    name::Name,
    serial_number::SerialNumber,
    spki::{EncodePublicKey, SubjectPublicKeyInfoOwned},
    time::Validity,
    Certificate,
};

use crate::{
    sign::{ecdsa, rsa, SigningKeyPair},
    CertificatePair, CertificatePairExt,
};

/// The root CA subject name containing the common name, organization name and
/// country.
pub const ROOT_CA_SUBJECT: &str = "CN=Stackable Root CA,O=Stackable GmbH,C=DE";

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to serialize certificate as PEM"))]
    SerializeCertificate { source: x509_cert::der::Error },

    #[snafu(display("failed to serialize private key as PEM"))]
    SerializePrivateKey { source: p256::pkcs8::Error },

    #[snafu(display("failed to write file"))]
    WriteFile { source: std::io::Error },
}

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

// TODO (@Techassi): Make this generic over the signing key used.
/// A certificate authority (CA) which is used to generate and sign
/// intermidiate or leaf certificates.
#[derive(Debug)]
pub struct CertificateAuthority<S>
where
    S: SigningKeyPair,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
    serial_numbers: Vec<u64>,
    certificate_pair: CertificatePair<S>,
}

impl<S> CertificatePairExt for CertificateAuthority<S>
where
    S: SigningKeyPair,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
    type Error = Error;

    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self, Self::Error> {
        todo!()
    }

    fn to_certificate_file(
        &self,
        certificate_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        let certificate_pem = self
            .certificate_pair
            .certificate
            .to_pem(line_ending)
            .context(SerializeCertificateSnafu)?;
        std::fs::write(certificate_path, certificate_pem).context(WriteFileSnafu)
    }

    fn to_private_key_file(
        &self,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        let private_key_pem = self
            .certificate_pair
            .signing_key
            .private_key()
            .to_pkcs8_pem(line_ending)
            .context(SerializePrivateKeySnafu)?;
        std::fs::write(private_key_path, private_key_pem).context(WriteFileSnafu)
    }
}

#[cfg(feature = "k8s")]
impl<S> K8sCertificatePair for CertificateAuthority<S>
where
    S: SigningKeyPair,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
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
            S::from_pkcs8_pem(std::str::from_utf8(&private_key_data.0).unwrap()).unwrap();

        Ok(Self {
            serial_numbers: Vec::new(),
            certificate_pair: CertificatePair {
                certificate,
                signing_key,
            },
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

impl<S> CertificateAuthority<S>
where
    S: SigningKeyPair,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Creates a new CA certificate which embeds the public part of the randomly
    /// generated signing key. The certificate is additionally signed by the
    /// private part of the signing key.
    #[instrument(name = "create_certificate_authority")]
    pub fn new(signing_key: S) -> Self {
        let serial_number = SerialNumber::from(rand::random::<u64>());
        let validity = Validity::from_now(Duration::from_secs(3600)).unwrap();
        let subject = Name::from_str(ROOT_CA_SUBJECT).unwrap();
        let spki = SubjectPublicKeyInfoOwned::from_pem(
            signing_key
                .public_key()
                .to_public_key_pem(LineEnding::default())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

        let signer = signing_key.private_key();

        let mut builder = CertificateBuilder::new(
            Profile::Root,
            serial_number,
            validity,
            subject,
            spki.clone(),
            signer,
        )
        .unwrap();

        // There are multiple default extensions included in the profile. For
        // the root profile, these are:
        //
        // - BasicConstraints marked as critical and CA = true
        // - SubjectKeyIdentifier with the 160-bit SHA-1 hash of the subject
        //   public key.
        // - KeyUsage with KeyCertSign and CRLSign bits set. Ideally we also
        //   want to include the DigitalSignature bit, which for example is
        //   required for CA certs which want to sign an OCSP response.
        //   Currently, the root profile doesn't include that bit.
        //
        // The root profile doesn't add the AuthorityKeyIdentifier extension.
        // We manually add it below by using the 160-bit SHA-1 hash of the
        // subject pulic key. This conforms to one of the outlined methods for
        // generating key identifiers outlined in RFC 5280, section 4.2.1.2.

        builder
            .add_extension(&AuthorityKeyIdentifier::try_from(spki.owned_to_ref()).unwrap())
            .unwrap();

        info!("create and sign CA certificate");
        let certificate = builder.build().unwrap();

        Self {
            serial_numbers: Vec::new(),
            certificate_pair: CertificatePair {
                certificate,
                signing_key,
            },
        }
    }

    #[instrument]
    pub fn generate_leaf_certificate(&self) -> Result<Certificate> {
        todo!()
    }
}

impl CertificateAuthority<rsa::SigningKey> {
    pub fn new_with_rsa() -> Self {
        // TODO (@Techassi): Remove unwrap
        CertificateAuthority::new(rsa::SigningKey::new().unwrap())
    }
}

impl CertificateAuthority<ecdsa::SigningKey> {
    pub fn new_with_ecdsa() -> Self {
        // TODO (@Techassi): Remove unwrap
        CertificateAuthority::new(ecdsa::SigningKey::new().unwrap())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::PathBufExt;

    use super::*;

    #[test]
    fn test() {
        let ca = CertificateAuthority::new_with_rsa();
        let (cert_path, pk_path) = PathBuf::certificate_pair_paths("ca");
        ca.to_files(cert_path, pk_path, LineEnding::default())
            .unwrap();
    }
}
