use std::fmt::Debug;

use bon::Builder;
use const_oid::db::rfc5280::{ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH};
use rsa::pkcs8::EncodePublicKey;
use snafu::{ResultExt, Snafu};
use stackable_operator::time::Duration;
use tracing::debug;
use x509_cert::{
    builder::{Builder, Profile},
    der::{DecodePem, asn1::Ia5String},
    ext::pkix::{ExtendedKeyUsage, SubjectAltName, name::GeneralName},
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
};

use crate::{
    CertificatePair,
    ca::{CertificateAuthority, DEFAULT_CERTIFICATE_VALIDITY, PEM_LINE_ENDING},
    keys::CertificateKeypair,
};

/// Defines all error variants which can occur when creating a CertificateRequest
#[derive(Debug, Snafu)]
pub enum CreateCertificateError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("failed to parse validity"))]
    ParseValidity { source: x509_cert::der::Error },

    #[snafu(display("failed to parse \"{subject}\" as subject"))]
    ParseSubject {
        source: x509_cert::der::Error,
        subject: String,
    },

    // #[snafu(display("failed to create key pair"))]
    // CreateKeyPair { source: Box<dyn std::error::Error> },
    #[snafu(display("failed to create key pair"))]
    CreateKeyPair { source: E },

    #[snafu(display("failed to serialize public key as PEM"))]
    SerializePublicKey { source: x509_cert::spki::Error },

    #[snafu(display("failed to decode SPKI from PEM"))]
    DecodeSpkiFromPem { source: x509_cert::der::Error },

    #[snafu(display("failed to create certificate builder"))]
    CreateCertificateBuilder { source: x509_cert::builder::Error },

    #[snafu(display("failed to add certificate extension"))]
    AddCertificateExtension { source: x509_cert::builder::Error },

    #[snafu(display("The subject alternative DNS name \"{dns_name}\" is not a Ia5String"))]
    SaDnsNameNotAIa5String {
        dns_name: String,
        source: x509_cert::der::Error,
    },

    #[snafu(display("failed to build certificate"))]
    BuildCertificate { source: x509_cert::builder::Error },
}

/// This builder builds certificates of type [`CertificatePair`].
///
/// Example code to construct a certificate:
///
/// ```no_run
/// use stackable_certs::{
///     keys::ecdsa,
///     ca::{CertificateAuthority, CertificateAuthorityBuilder},
///     CertificateBuilder,
/// };
///
/// let ca: CertificateAuthority<ecdsa::SigningKey> = CertificateAuthorityBuilder::builder()
///     .build()
///     .build_ca()
///     .expect("failed to build CA");
///
/// let certificate = CertificateBuilder::builder()
///     .subject("CN=trino-coordinator-default-0")
///     .signed_by(&ca)
///     .build()
///     .build_certificate()
///     .expect("failed to build certificate");
/// ```
#[derive(Builder)]
pub struct CertificateBuilder<'a, KP>
where
    KP: CertificateKeypair,
    <KP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Required subject of the certificate, usually starts with `CN=`.
    subject: &'a str,

    /// Optional list of subject alternative names (SAN) DNS entries,
    /// that are added to the certificate.
    #[builder(default)]
    subject_alterative_dns_names: &'a [&'a str],

    /// Validity/lifetime of the certificate.
    ///
    /// If not specified the default of [`DEFAULT_CERTIFICATE_VALIDITY`] will be used.
    #[builder(default = DEFAULT_CERTIFICATE_VALIDITY)]
    validity: Duration,

    /// Serial number of the generated certificate.
    ///
    /// If not specified a random serial will be generated.
    serial_number: Option<u64>,

    /// Cryptographic keypair used to for the certificates.
    ///
    /// If not specified a random keypair will be generated.
    key_pair: Option<KP>,

    /// Mandatorily sign the certificate using the provided [`CertificateAuthority`].
    signed_by: &'a CertificateAuthority<KP>,
}

impl<KP> CertificateBuilder<'_, KP>
where
    KP: CertificateKeypair,
    <KP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    pub fn build_certificate(
        self,
    ) -> Result<CertificatePair<KP>, CreateCertificateError<KP::Error>> {
        let serial_number =
            SerialNumber::from(self.serial_number.unwrap_or_else(|| rand::random::<u64>()));
        // NOTE (@Techassi): Should we validate that the validity is shorter
        // than the validity of the issuing CA?
        let validity = Validity::from_now(*self.validity).context(ParseValiditySnafu)?;
        let subject: Name = self.subject.parse().context(ParseSubjectSnafu {
            subject: self.subject,
        })?;
        let key_pair = match self.key_pair {
            Some(key_pair) => key_pair,
            None => KP::new().context(CreateKeyPairSnafu)?,
        };

        let spki_pem = key_pair
            .verifying_key()
            .to_public_key_pem(PEM_LINE_ENDING)
            .context(SerializePublicKeySnafu)?;

        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes())
            .context(DecodeSpkiFromPemSnafu)?;

        let signing_key = self.signed_by.signing_key();
        let mut builder = x509_cert::builder::CertificateBuilder::new(
            Profile::Leaf {
                issuer: self.signed_by.issuer_name().clone(),
                enable_key_agreement: false,
                enable_key_encipherment: true,
            },
            serial_number,
            validity,
            subject,
            spki,
            signing_key,
        )
        .context(CreateCertificateBuilderSnafu)?;

        // The leaf certificate can be used for WWW client and server
        // authentication. This is a base requirement for TLS certs.
        builder
            .add_extension(&ExtendedKeyUsage(vec![
                ID_KP_CLIENT_AUTH,
                ID_KP_SERVER_AUTH,
            ]))
            .context(AddCertificateExtensionSnafu)?;

        let sans = self
            .subject_alterative_dns_names
            .iter()
            .map(|dns_name| {
                Ok(GeneralName::DnsName(Ia5String::new(dns_name).context(
                    SaDnsNameNotAIa5StringSnafu {
                        dns_name: dns_name.to_string(),
                    },
                )?))
            })
            .collect::<Result<Vec<_>, CreateCertificateError<KP::Error>>>()?;
        builder
            .add_extension(&SubjectAltName(sans))
            .context(AddCertificateExtensionSnafu)?;

        debug!("create and sign leaf certificate");
        let certificate = builder.build().context(BuildCertificateSnafu)?;

        Ok(CertificatePair {
            certificate,
            key_pair,
        })
    }
}

#[cfg(test)]
mod tests {
    use x509_cert::{
        certificate::TbsCertificateInner, der::Decode, ext::pkix::ID_CE_SUBJECT_ALT_NAME,
    };

    use super::*;
    use crate::{
        ca::CertificateAuthorityBuilder,
        keys::{ecdsa, rsa},
    };

    #[test]
    fn minimal_certificate() {
        let ca = get_ecdsa_ca();
        let certificate = CertificateBuilder::builder()
            .subject("CN=trino-coordinator-default-0")
            .signed_by(&ca)
            .build()
            .build_certificate()
            .expect("failed to build certificate");

        assert_certificate_attributes(
            &certificate.certificate.tbs_certificate,
            "CN=trino-coordinator-default-0",
            &[],
            DEFAULT_CERTIFICATE_VALIDITY,
            None,
        );
    }

    #[test]
    fn customized_certificate() {
        let ca = get_rsa_ca();
        let sans = [
            "trino-coordinator-default-0.trino-coordinator-default.default.svc.cluster-local",
            "trino-coordinator-default.default.svc.cluster-local",
        ];
        let certificate = CertificateBuilder::builder()
            .subject("CN=trino-coordinator-default-0")
            .subject_alterative_dns_names(&sans)
            .serial_number(08121997)
            .validity(Duration::from_days_unchecked(42))
            .key_pair(rsa::SigningKey::new().unwrap())
            .signed_by(&ca)
            .build()
            .build_certificate()
            .expect("failed to build certificate");

        assert_certificate_attributes(
            &certificate.certificate.tbs_certificate,
            "CN=trino-coordinator-default-0",
            &sans,
            Duration::from_days_unchecked(42),
            Some(08121997),
        );
    }

    fn assert_certificate_attributes(
        certificate: &TbsCertificateInner,
        subject: &str,
        sans: &[&str],
        validity: Duration,
        serial_number: Option<u64>,
    ) {
        assert_eq!(certificate.subject, subject.parse().unwrap());

        let extensions = certificate
            .extensions
            .as_ref()
            .expect("cert had no extension");
        let san_extension = extensions
            .iter()
            .find(|ext| ext.extn_id == ID_CE_SUBJECT_ALT_NAME)
            .expect("cert had no SAN extension");

        let actual_sans = SubjectAltName::from_der(san_extension.extn_value.as_bytes())
            .expect("failed to parse SAN");
        let actual_sans = actual_sans
            .0
            .iter()
            .filter_map(|san| match san {
                GeneralName::DnsName(dns_name) => Some(dns_name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_sans, sans);

        let not_before = certificate.validity.not_before.to_system_time();
        let not_after = certificate.validity.not_after.to_system_time();
        assert_eq!(
            not_after
                .duration_since(not_before)
                .expect("Failed to calculate duration between notBefore and notAfter"),
            *validity
        );

        if let Some(serial_number) = serial_number {
            assert_eq!(certificate.serial_number, SerialNumber::from(serial_number))
        } else {
            assert_ne!(certificate.serial_number, SerialNumber::from(0_u64))
        }
    }

    fn get_ecdsa_ca() -> CertificateAuthority<ecdsa::SigningKey> {
        CertificateAuthorityBuilder::builder()
            .build()
            .build_ca()
            .expect("failed to build CA")
    }

    fn get_rsa_ca() -> CertificateAuthority<rsa::SigningKey> {
        CertificateAuthorityBuilder::builder()
            .build()
            .build_ca()
            .expect("failed to build CA")
    }
}
