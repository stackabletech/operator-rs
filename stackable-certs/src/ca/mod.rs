//! Contains types and functions to generate and sign certificate authorities
//! (CAs).
use std::{collections::HashSet, path::Path, str::FromStr};

#[cfg(feature = "k8s")]
use {
    crate::K8sCertificatePair,
    k8s_openapi::api::core::v1::Secret,
    kube::ResourceExt,
    stackable_operator::{client::Client, commons::secret::SecretReference},
};

use const_oid::db::rfc5280::{ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH};
use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::time::Duration;
use tracing::{info, instrument};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, referenced::OwnedToRef, DecodePem},
    ext::pkix::{AuthorityKeyIdentifier, ExtendedKeyUsage},
    name::Name,
    serial_number::SerialNumber,
    spki::{EncodePublicKey, SubjectPublicKeyInfoOwned},
    time::Validity,
    Certificate,
};

use crate::{
    keys::{ecdsa, rsa, KeypairExt},
    CertificatePair, CertificatePairError, CertificatePairExt,
};

mod consts;
pub use consts::*;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to generate RSA signing key"))]
    GenerateRsaSigningKey { source: rsa::Error },

    #[snafu(display("failed to generate ECDSA signign key"))]
    GenerateEcdsaSigningKey { source: ecdsa::Error },

    #[snafu(display("failed to generate a unique serial number after 5 tries"))]
    GenerateUniqueSerialNumber,

    #[snafu(display("failed to parse {subject:?} as subject"))]
    InvalidSubject {
        source: x509_cert::der::Error,
        subject: String,
    },
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
    ReadSigningKey { source: rsa::Error, secret: String },
}

// TODO (@Techassi): Make this generic over the signing key used.
/// A certificate authority (CA) which is used to generate and sign
/// intermidiate or leaf certificates.
#[derive(Debug)]
pub struct CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate_pair: CertificatePair<S>,
    serial_numbers: HashSet<u64>,
}

impl<S> CertificatePairExt for CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    type Error = CertificatePairError;

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
        self.certificate_pair
            .to_certificate_file(certificate_path, line_ending)
    }

    fn to_private_key_file(
        &self,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error> {
        self.certificate_pair
            .to_private_key_file(private_key_path, line_ending)
    }
}

#[cfg(feature = "k8s")]
impl<S> K8sCertificatePair for CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
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
        let signing_key_pair =
            S::from_pkcs8_pem(std::str::from_utf8(&private_key_data.0).unwrap()).unwrap();

        Ok(Self {
            serial_numbers: HashSet::new(),
            certificate_pair: CertificatePair {
                key_pair: signing_key_pair,
                certificate,
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
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    // TODO (@Techassi): Add doc comment
    #[instrument(name = "create_certificate_authority", skip(signing_key_pair))]
    pub fn new(signing_key_pair: S) -> Result<Self> {
        let serial_number = rand::random::<u64>();
        let validity = Duration::from_secs(3600);

        Self::new_with(signing_key_pair, serial_number, validity)
    }

    // TODO (@Techassi): Adjust doc comment
    /// Creates a new CA certificate identified by a randomly generated serial
    /// number. It contains the public half of the provided `signing_key`.
    /// Furthermore, it is signed by the private half of the signing key.
    #[instrument(name = "create_certificate_authority_with", skip(signing_key_pair))]
    pub fn new_with(signing_key_pair: S, serial_number: u64, validity: Duration) -> Result<Self> {
        let serial_number = SerialNumber::from(serial_number);
        let validity = Validity::from_now(*validity).unwrap();
        let subject = Name::from_str(ROOT_CA_SUBJECT).unwrap();
        let spki_pem = signing_key_pair
            .verifying_key()
            .to_public_key_pem(LineEnding::default())
            .unwrap();
        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes()).unwrap();

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
        //
        // Now first prepare extensions so we can avoid clones.
        let aki = AuthorityKeyIdentifier::try_from(spki.owned_to_ref()).unwrap();

        let signer = signing_key_pair.signing_key();
        let mut builder = CertificateBuilder::new(
            Profile::Root,
            serial_number,
            validity,
            subject,
            spki,
            signer,
        )
        .unwrap();

        // Add extension constructed above
        builder.add_extension(&aki).unwrap();

        info!("create and sign CA certificate");
        let certificate = builder.build().unwrap();

        Ok(Self {
            serial_numbers: HashSet::new(),
            certificate_pair: CertificatePair {
                key_pair: signing_key_pair,
                certificate,
            },
        })
    }

    #[instrument(skip_all)]
    pub fn generate_leaf_certificate<T>(
        &mut self,
        key_pair: T,
        name: &str,
        scope: &str,
    ) -> Result<CertificatePair<T>>
    where
        T: KeypairExt,
        <T::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
    {
        // TODO (@Techassi): Remove all unwraps below
        let serial_number = self.generate_serial_number()?;
        let validity = Validity::from_now(*Duration::from_secs(3600)).unwrap(); // TODO (@Techassi): Make configurable
        let subject = format_leaf_certificate_subject(name, scope)?;
        let spki_pem = key_pair
            .verifying_key()
            .to_public_key_pem(LineEnding::default())
            .unwrap();
        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes()).unwrap();

        let eku = ExtendedKeyUsage(vec![ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH]);
        let aki = AuthorityKeyIdentifier::try_from(spki.owned_to_ref()).unwrap();

        let signer = self.certificate_pair.key_pair.signing_key();
        let mut builder = CertificateBuilder::new(
            Profile::Leaf {
                issuer: self
                    .certificate_pair
                    .certificate
                    .tbs_certificate
                    .issuer
                    .clone(),
                enable_key_agreement: false,
                enable_key_encipherment: true,
            },
            serial_number,
            validity,
            subject,
            spki,
            signer,
        )
        .unwrap();

        builder.add_extension(&eku).unwrap();
        builder.add_extension(&aki).unwrap();

        info!("create and sign leaf certificate");
        let certificate = builder.build().unwrap();

        Ok(CertificatePair {
            certificate,
            key_pair,
        })
    }

    #[instrument(skip_all)]
    fn generate_serial_number(&mut self) -> Result<SerialNumber> {
        let mut serial_number = rand::random();
        let mut tries = 0;

        while self.serial_numbers.contains(&serial_number) {
            if tries >= 5 {
                return GenerateUniqueSerialNumberSnafu.fail();
            }

            serial_number = rand::random();
            tries += 1;
        }

        Ok(SerialNumber::from(serial_number))
    }
}

impl CertificateAuthority<rsa::SigningKey> {
    #[instrument(name = "create_certificate_authority_with_rsa")]
    pub fn new_rsa() -> Result<Self> {
        Self::new(rsa::SigningKey::new(None).context(GenerateRsaSigningKeySnafu)?)
    }
}

impl CertificateAuthority<ecdsa::SigningKey> {
    #[instrument(name = "create_certificate_authority_with_ecdsa")]
    pub fn new_ecdsa() -> Result<Self> {
        Self::new(ecdsa::SigningKey::new().context(GenerateEcdsaSigningKeySnafu)?)
    }
}

fn format_leaf_certificate_subject(name: &str, scope: &str) -> Result<Name> {
    let subject = format!("CN={name} Certificate for {scope},{ORGANIZATION_DN},{COUNTRY_DN}");
    Name::from_str(&subject).context(InvalidSubjectSnafu { subject })
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::PathBufExt;

    use super::*;

    #[test]
    fn test() {
        let mut ca = CertificateAuthority::new_rsa().unwrap();
        ca.generate_leaf_certificate(rsa::SigningKey::new(None).unwrap(), "Airflow", "pod")
            .unwrap()
            .to_certificate_file(PathBuf::certificate_path("tls"), LineEnding::default())
            .unwrap();
    }
}
