//! Contains types and functions to generate and sign certificate authorities
//! (CAs).
use std::str::FromStr;

use const_oid::db::rfc5280::{ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH};
use k8s_openapi::api::core::v1::Secret;
use kube::runtime::reflector::ObjectRef;
use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::{client::Client, commons::secret::SecretReference, time::Duration};
use tracing::{debug, instrument};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{pem::LineEnding, referenced::OwnedToRef, DecodePem},
    ext::pkix::{AuthorityKeyIdentifier, ExtendedKeyUsage},
    name::Name,
    serial_number::SerialNumber,
    spki::{EncodePublicKey, SubjectPublicKeyInfoOwned},
    time::Validity,
};

use crate::{
    keys::{ecdsa, rsa, KeypairExt},
    CertificatePair,
};

mod consts;
pub use consts::*;

pub const TLS_SECRET_TYPE: &str = "kubernetes.io/tls";

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Defines all error variants which can occur when creating a CA and/or leaf
/// certificates.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to generate RSA signing key"))]
    GenerateRsaSigningKey { source: rsa::Error },

    #[snafu(display("failed to generate ECDSA signign key"))]
    GenerateEcdsaSigningKey { source: ecdsa::Error },

    #[snafu(display("failed to parse {subject:?} as subject"))]
    ParseSubject {
        source: x509_cert::der::Error,
        subject: String,
    },

    #[snafu(display("failed to parse validity"))]
    ParseValidity { source: x509_cert::der::Error },

    #[snafu(display("failed to serialize public key as PEM"))]
    SerializePublicKey { source: x509_cert::spki::Error },

    #[snafu(display("failed to decode SPKI from PEM"))]
    DecodeSpkiFromPem { source: x509_cert::der::Error },

    #[snafu(display("failed to create certificate builder"))]
    CreateCertificateBuilder { source: x509_cert::builder::Error },

    #[snafu(display("failed to add certificate extension"))]
    AddCertificateExtension { source: x509_cert::builder::Error },

    #[snafu(display("failed to build certificate"))]
    BuildCertificate { source: x509_cert::builder::Error },

    #[snafu(display("failed to parse AuthorityKeyIdentifier"))]
    ParseAuthorityKeyIdentifier { source: x509_cert::der::Error },
}

/// Defines all error variants which can occur when loading a CA from a
/// Kubernetes [`Secret`].
#[derive(Debug, Snafu)]
pub enum SecretError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("failed to retrieve secret {secret:?}"))]
    GetSecret { source: kube::Error, secret: String },

    #[snafu(display("invalid secret type, expected {TLS_SECRET_TYPE}"))]
    InvalidSecretType,

    #[snafu(display("the secret {secret:?} does not contain any data"))]
    NoSecretData { secret: ObjectRef<Secret> },

    #[snafu(display("the secret {secret:?} does not contain TLS certificate data"))]
    NoCertificateData { secret: ObjectRef<Secret> },

    #[snafu(display("the secret {secret:?} does not contain TLS private key data"))]
    NoPrivateKeyData { secret: ObjectRef<Secret> },

    #[snafu(display("failed to read PEM-encoded certificate chain from secret {secret:?}"))]
    ReadChain {
        source: x509_cert::der::Error,
        secret: ObjectRef<Secret>,
    },

    #[snafu(display("failed to parse Base64 encoded byte string"))]
    DecodeUtf8String { source: std::str::Utf8Error },

    #[snafu(display("failed to deserialize private key from PEM"))]
    DeserializeKeyFromPem { source: E },
}

/// A certificate authority (CA) which is used to generate and sign
/// intermidiate or leaf certificates.
#[derive(Debug)]
pub struct CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate_pair: CertificatePair<S>,
}

impl<S> CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Creates a new CA certificate with many parameters set to their default
    /// values.
    ///
    /// These parameters include:
    ///
    /// - a randomly generated serial number
    /// - a default validity of one hour (see [`DEFAULT_CA_VALIDITY_SECONDS`])
    ///
    /// The CA contains the public half of the provided `signing_key` and is
    /// signed by the private half of said key.
    ///
    /// If the default values for the serial number and validity don't satisfy
    /// the requirements of the caller, use [`CertificateAuthority::new_with`]
    /// instead.
    #[instrument(name = "create_certificate_authority", skip(signing_key_pair))]
    pub fn new(signing_key_pair: S) -> Result<Self> {
        let serial_number = rand::random::<u64>();
        let validity = Duration::from_secs(DEFAULT_CA_VALIDITY_SECONDS);

        Self::new_with(signing_key_pair, serial_number, validity)
    }

    /// Creates a new CA certificate.
    ///
    /// Instead of providing sensible defaults for the serial number and
    /// validity, this function offers complete control over these parameters.
    /// If this level of control is not needed, use [`CertificateAuthority::new`]
    /// instead.
    #[instrument(name = "create_certificate_authority_with", skip(signing_key_pair))]
    pub fn new_with(signing_key_pair: S, serial_number: u64, validity: Duration) -> Result<Self> {
        let serial_number = SerialNumber::from(serial_number);
        let validity = Validity::from_now(*validity).context(ParseValiditySnafu)?;

        // We don't allow customization of the CA subject by callers. Every CA
        // created by us should contain the same subject consisting a common set
        // of distinguished names (DNs).
        let subject = Name::from_str(ROOT_CA_SUBJECT).context(ParseSubjectSnafu {
            subject: ROOT_CA_SUBJECT,
        })?;

        let spki_pem = signing_key_pair
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .context(SerializePublicKeySnafu)?;

        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes())
            .context(DecodeSpkiFromPemSnafu)?;

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
        // Prepare extensions so we can avoid clones.
        let aki = AuthorityKeyIdentifier::try_from(spki.owned_to_ref())
            .context(ParseAuthorityKeyIdentifierSnafu)?;

        let signer = signing_key_pair.signing_key();
        let mut builder = CertificateBuilder::new(
            Profile::Root,
            serial_number,
            validity,
            subject,
            spki,
            signer,
        )
        .context(CreateCertificateBuilderSnafu)?;

        // Add extension constructed above
        builder
            .add_extension(&aki)
            .context(AddCertificateExtensionSnafu)?;

        debug!("create and sign CA certificate");
        let certificate = builder.build().context(BuildCertificateSnafu)?;

        Ok(Self {
            certificate_pair: CertificatePair {
                key_pair: signing_key_pair,
                certificate,
            },
        })
    }

    /// Generates a leaf certificate which is signed by this CA.
    ///
    /// The certificate requires a `name` and a `scope`. Both these values
    /// are part of the certificate subject. The format is: `{name} Certificate
    /// for {scope}`. These leaf certificates can be used for client/server
    /// authentication, because they include [`ID_KP_CLIENT_AUTH`] and
    /// [`ID_KP_SERVER_AUTH`] in the extended key usage extension.
    ///
    /// It is also possible to directly greate RSA or ECDSA-based leaf
    /// certificates using [`CertificateAuthority::generate_rsa_leaf_certificate`]
    /// and [`CertificateAuthority::generate_ecdsa_leaf_certificate`].
    #[instrument(skip(key_pair))]
    pub fn generate_leaf_certificate<T>(
        &mut self,
        key_pair: T,
        name: &str,
        scope: &str,
        validity: Duration,
    ) -> Result<CertificatePair<T>>
    where
        T: KeypairExt,
        <T::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
    {
        // We generate a random serial number, but ensure the same CA didn't
        // issue another certificate with the same serial number. We try to
        // generate a unique serial number at max five times before giving up
        // and returning an error.
        let serial_number = SerialNumber::from(rand::random::<u64>());

        // NOTE (@Techassi): Should we validate that the validity is shorter
        // than the validity of the issuing CA?
        let validity = Validity::from_now(*validity).context(ParseValiditySnafu)?;
        let subject = format_leaf_certificate_subject(name, scope)?;

        let spki_pem = key_pair
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .context(SerializePublicKeySnafu)?;

        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes())
            .context(DecodeSpkiFromPemSnafu)?;

        // The leaf certificate can be used for WWW client and server
        // authentication. This is a base requirement for TLS certs.
        let eku = ExtendedKeyUsage(vec![ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH]);
        let aki = AuthorityKeyIdentifier::try_from(spki.owned_to_ref())
            .context(ParseAuthorityKeyIdentifierSnafu)?;

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
        .context(CreateCertificateBuilderSnafu)?;

        // Again, add the extension created above.
        builder
            .add_extension(&eku)
            .context(AddCertificateExtensionSnafu)?;
        builder
            .add_extension(&aki)
            .context(AddCertificateExtensionSnafu)?;

        debug!("create and sign leaf certificate");
        let certificate = builder.build().context(BuildCertificateSnafu)?;

        Ok(CertificatePair {
            certificate,
            key_pair,
        })
    }

    /// Generates an RSA-based leaf certificate which is signed by this CA.
    ///
    /// See [`CertificateAuthority::generate_leaf_certificate`] for more
    /// information.
    #[instrument]
    pub fn generate_rsa_leaf_certificate(
        &mut self,
        bit_size: rsa::BitSize,
        name: &str,
        scope: &str,
        validity: Duration,
    ) -> Result<CertificatePair<rsa::SigningKey>> {
        let key = rsa::SigningKey::new(bit_size).context(GenerateRsaSigningKeySnafu)?;
        self.generate_leaf_certificate(key, name, scope, validity)
    }

    /// Generates an ECDSAasync -based leaf certificate which is signed by this CA.
    ///
    /// See [`CertificateAuthority::generate_leaf_certificate`] for more
    /// information.
    #[instrument]
    pub fn generate_ecdsa_leaf_certificate(
        &mut self,
        name: &str,
        scope: &str,
        validity: Duration,
    ) -> Result<CertificatePair<ecdsa::SigningKey>> {
        let key = ecdsa::SigningKey::new().context(GenerateEcdsaSigningKeySnafu)?;
        self.generate_leaf_certificate(key, name, scope, validity)
    }

    /// Create a [`CertificateAuthority`] from a Kubernetes [`Secret`].
    #[instrument(name = "create_certificate_authority_from_k8s_secret", skip(secret))]
    pub fn from_secret(
        secret: Secret,
        key_certificate: &str,
        key_private_key: &str,
    ) -> Result<Self, SecretError<S::Error>> {
        if !secret.type_.as_ref().is_some_and(|s| s == TLS_SECRET_TYPE) {
            return InvalidSecretTypeSnafu.fail();
        }

        let data = secret.data.as_ref().with_context(|| NoSecretDataSnafu {
            secret: ObjectRef::from_obj(&secret),
        })?;

        debug!("retrieving certificate data from secret via key {key_certificate:?}");
        let certificate_data =
            data.get(key_certificate)
                .with_context(|| NoCertificateDataSnafu {
                    secret: ObjectRef::from_obj(&secret),
                })?;

        let certificate = x509_cert::Certificate::load_pem_chain(&certificate_data.0)
            .with_context(|_| ReadChainSnafu {
                secret: ObjectRef::from_obj(&secret),
            })?
            .remove(0);

        debug!("retrieving private key data from secret via key {key_certificate:?}");
        let private_key_data =
            data.get(key_private_key)
                .with_context(|| NoPrivateKeyDataSnafu {
                    secret: ObjectRef::from_obj(&secret),
                })?;

        let private_key_data =
            std::str::from_utf8(&private_key_data.0).context(DecodeUtf8StringSnafu)?;

        let signing_key_pair =
            S::from_pkcs8_pem(private_key_data).context(DeserializeKeyFromPemSnafu)?;

        Ok(Self {
            certificate_pair: CertificatePair {
                key_pair: signing_key_pair,
                certificate,
            },
        })
    }

    /// Create a [`CertificateAuthority`] from a Kubernetes [`SecretReference`].
    #[instrument(
        name = "create_certificate_authority_from_k8s_secret_ref",
        skip(secret_ref, client)
    )]
    pub async fn from_secret_ref(
        secret_ref: &SecretReference,
        key_certificate: &str,
        key_private_key: &str,
        client: &Client,
    ) -> Result<Self, SecretError<S::Error>> {
        let secret_api = client.get_api::<Secret>(&secret_ref.namespace);
        let secret = secret_api
            .get(&secret_ref.name)
            .await
            .context(GetSecretSnafu {
                secret: secret_ref.name.clone(),
            })?;

        Self::from_secret(secret, key_certificate, key_private_key)
    }
}

impl CertificateAuthority<rsa::SigningKey> {
    /// High-level function to create a new CA using a RSA key pair.
    #[instrument(name = "create_certificate_authority_with_rsa")]
    pub fn new_rsa() -> Result<Self> {
        Self::new(rsa::SigningKey::new(rsa::BitSize::Default).context(GenerateRsaSigningKeySnafu)?)
    }
}

impl CertificateAuthority<ecdsa::SigningKey> {
    /// High-level function to create a new CA using a ECDSA key pair.
    #[instrument(name = "create_certificate_authority_with_ecdsa")]
    pub fn new_ecdsa() -> Result<Self> {
        Self::new(ecdsa::SigningKey::new().context(GenerateEcdsaSigningKeySnafu)?)
    }
}

fn format_leaf_certificate_subject(name: &str, scope: &str) -> Result<Name> {
    let subject = format!("CN={name} Certificate for {scope}");
    Name::from_str(&subject).context(ParseSubjectSnafu { subject })
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn test() {
        let mut ca = CertificateAuthority::new_rsa().unwrap();
        ca.generate_leaf_certificate(
            rsa::SigningKey::new(rsa::BitSize::Default).unwrap(),
            "Airflow",
            "pod",
            Duration::from_secs(3600),
        )
        .unwrap();
    }
}
