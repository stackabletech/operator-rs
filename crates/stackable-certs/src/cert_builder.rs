use std::{fmt::Debug, net::IpAddr, time::SystemTime};

use bon::Builder;
use const_oid::db::rfc5280::{ID_KP_CLIENT_AUTH, ID_KP_SERVER_AUTH};
use rsa::pkcs8::EncodePublicKey;
use snafu::{ResultExt, Snafu, ensure};
use stackable_operator::time::Duration;
use tracing::{debug, instrument, warn};
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

/// Defines all error variants which can occur when creating a certificate
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

    #[snafu(display(
        "failed to parse subject alternative DNS name \"{subject_alternative_dns_name}\" as a Ia5 string"
    ))]
    ParseSubjectAlternativeDnsName {
        subject_alternative_dns_name: String,
        source: x509_cert::der::Error,
    },

    #[snafu(display("failed to build certificate"))]
    BuildCertificate { source: x509_cert::builder::Error },

    #[snafu(display(
        "the generated certificate would outlive the CA, subject {subject:?}, \
        CA notAfter {ca_not_after:?}, CA notBefore {ca_not_before:?}, \
        cert notAfter {cert_not_after:?}, cert notBefore {cert_not_before:?}"
    ))]
    CertOutlivesCa {
        subject: String,
        ca_not_after: SystemTime,
        ca_not_before: SystemTime,
        cert_not_after: SystemTime,
        cert_not_before: SystemTime,
    },
}

/// This builder builds certificates of type [`CertificatePair`].
///
/// Currently you are required to specify a [`CertificateAuthority`], which is used to create a leaf
/// certificate, which is signed by this CA.
///
/// These leaf certificates can be used for client/server authentication, because they include
/// [`ID_KP_CLIENT_AUTH`] and [`ID_KP_SERVER_AUTH`] in the extended key usage extension.
///
/// This builder has many default values, notably;
///
/// - A default validity of [`DEFAULT_CERTIFICATE_VALIDITY`]
/// - A randomly generated serial number
/// - In case no `key_pair` was provided, a fresh keypair will be created. The algorithm
///   (`rsa`/`ecdsa`) is chosen by the generic [`CertificateKeypair`] type of this struct,
///   which is normally inferred from the [`CertificateAuthority`].
///
/// Example code to construct a CA and a signed certificate:
///
/// ```no_run
/// use stackable_certs::{
///     keys::ecdsa,
///     ca::CertificateAuthority,
///     CertificatePair,
/// };
///
/// let ca = CertificateAuthority::<ecdsa::SigningKey>::builder()
///     .build()
///     .expect("failed to build CA");
///
/// let certificate = CertificatePair::builder()
///     .subject("CN=trino-coordinator-default-0")
///     .signed_by(&ca)
///     .build()
///     .expect("failed to build certificate");
/// ```
#[derive(Builder)]
#[builder(start_fn = start_builder, finish_fn = finish_builder)]
pub struct CertificateBuilder<'a, KP>
where
    KP: CertificateKeypair,
    <KP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Required subject of the certificate, usually starts with `CN=`, e.g. `CN=mypod`.
    subject: &'a str,

    /// Optional list of subject alternative name DNS entries
    /// that are added to the certificate.
    #[builder(default)]
    subject_alternative_dns_names: &'a [&'a str],

    /// Optional list of subject alternative name IP address entries
    /// that are added to the certificate.
    #[builder(default)]
    subject_alternative_ip_addresses: &'a [IpAddr],

    /// Validity/lifetime of the certificate.
    ///
    /// If not specified the default of [`DEFAULT_CERTIFICATE_VALIDITY`] will be used.
    #[builder(default = DEFAULT_CERTIFICATE_VALIDITY)]
    validity: Duration,

    /// Cryptographic keypair used to for the certificates.
    ///
    /// If not specified a random keypair will be generated.
    key_pair: Option<KP>,

    /// Mandatorily sign the certificate using the provided [`CertificateAuthority`].
    signed_by: &'a CertificateAuthority<KP>,
}

impl<KP, S> CertificateBuilderBuilder<'_, KP, S>
where
    KP: CertificateKeypair,
    <KP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
    S: certificate_builder_builder::IsComplete,
{
    /// Convenience function to avoid calling `builder().finish_builder().build()`
    pub fn build(self) -> Result<CertificatePair<KP>, CreateCertificateError<KP::Error>> {
        self.finish_builder().build()
    }
}

impl<SKP> CertificateBuilder<'_, SKP>
where
    SKP: CertificateKeypair,
    <SKP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    #[instrument(
        name = "build_certificate",
        skip(self),
        fields(subject = self.subject),
    )]
    pub fn build(self) -> Result<CertificatePair<SKP>, CreateCertificateError<SKP::Error>> {
        let validity = Validity::from_now(*self.validity).context(ParseValiditySnafu)?;
        let subject_for_error = &self.subject;
        let subject: Name = self.subject.parse().context(ParseSubjectSnafu {
            subject: self.subject,
        })?;
        let key_pair = match self.key_pair {
            Some(key_pair) => key_pair,
            None => SKP::new().context(CreateKeyPairSnafu)?,
        };

        // By choosing a random serial number we can make the reasonable assumption that we generate
        // a unique serial for each certificate.
        let serial_number = SerialNumber::from(rand::random::<u64>());

        let ca_validity = self.signed_by.ca_cert().tbs_certificate.validity;
        let ca_not_after = ca_validity.not_after.to_system_time();
        let ca_not_before = ca_validity.not_before.to_system_time();
        let cert_not_after = validity.not_after.to_system_time();
        let cert_not_before = validity.not_before.to_system_time();

        ensure!(ca_not_after > cert_not_after, CertOutlivesCaSnafu {
            subject: subject_for_error.to_string(),
            ca_not_after,
            ca_not_before,
            cert_not_after,
            cert_not_before,
        });

        let spki_pem = key_pair
            .verifying_key()
            .to_public_key_pem(PEM_LINE_ENDING)
            .context(SerializePublicKeySnafu)?;

        let spki = SubjectPublicKeyInfoOwned::from_pem(spki_pem.as_bytes())
            .context(DecodeSpkiFromPemSnafu)?;

        debug!(
            certificate.subject = %subject,
            certificate.not_after = %validity.not_after,
            certificate.not_before = %validity.not_before,
            certificate.serial = %serial_number,
            certificate.san.dns_names = ?self.subject_alternative_dns_names,
            certificate.san.ip_addresses = ?self.subject_alternative_ip_addresses,
            certificate.signed_by.issuer = %self.signed_by.issuer_name(),
            certificate.public_key.algorithm = SKP::algorithm_name(),
            certificate.public_key.size = SKP::key_size(),
            "creating and signing certificate"
        );
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

        let san_dns = self.subject_alternative_dns_names.iter().map(|dns_name| {
            Ok(GeneralName::DnsName(
                Ia5String::new(dns_name).with_context(|_| ParseSubjectAlternativeDnsNameSnafu {
                    subject_alternative_dns_name: dns_name.to_string(),
                })?,
            ))
        });
        let san_ips = self
            .subject_alternative_ip_addresses
            .iter()
            .copied()
            .map(GeneralName::from)
            .map(Result::Ok);
        let sans = san_dns
            .chain(san_ips)
            .collect::<Result<Vec<_>, CreateCertificateError<SKP::Error>>>()?;

        builder
            .add_extension(&SubjectAltName(sans))
            .context(AddCertificateExtensionSnafu)?;

        let certificate = builder.build().context(BuildCertificateSnafu)?;

        Ok(CertificatePair {
            certificate,
            key_pair,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use x509_cert::{
        certificate::TbsCertificateInner, der::Decode, ext::pkix::ID_CE_SUBJECT_ALT_NAME,
    };

    use super::*;
    use crate::keys::rsa;

    #[test]
    fn minimal_certificate() {
        let ca = CertificateAuthority::builder_with_ecdsa()
            .build()
            .expect("failed to build CA");

        let certificate = CertificatePair::builder()
            .subject("CN=trino-coordinator-default-0")
            .signed_by(&ca)
            .build()
            .expect("failed to build certificate");

        assert_certificate_attributes(
            &certificate.certificate.tbs_certificate,
            "CN=trino-coordinator-default-0",
            &[],
            &[],
            DEFAULT_CERTIFICATE_VALIDITY,
        );
    }

    #[test]
    fn customized_certificate() {
        let ca = CertificateAuthority::builder_with_rsa()
            .build()
            .expect("failed to build CA");

        let sans = [
            "trino-coordinator-default-0.trino-coordinator-default.default.svc.cluster-local",
            "trino-coordinator-default.default.svc.cluster-local",
        ];
        let san_ips = ["10.0.0.1".parse().unwrap(), "fe80::42".parse().unwrap()];

        let certificate = CertificatePair::builder()
            .subject("CN=trino-coordinator-default-0")
            .subject_alternative_dns_names(&sans)
            .subject_alternative_ip_addresses(&san_ips)
            .validity(Duration::from_hours_unchecked(12))
            .key_pair(rsa::SigningKey::new().unwrap())
            .signed_by(&ca)
            .build()
            .expect("failed to build certificate");

        assert_certificate_attributes(
            &certificate.certificate.tbs_certificate,
            "CN=trino-coordinator-default-0",
            &sans,
            &san_ips,
            Duration::from_hours_unchecked(12),
        );
    }

    #[test]
    fn cert_outlives_ca() {
        let ca = CertificateAuthority::builder_with_ecdsa()
            .validity(Duration::from_days_unchecked(365))
            .build()
            .expect("failed to build CA");

        let err = CertificatePair::builder()
            .subject("CN=Test")
            .signed_by(&ca)
            .validity(Duration::from_days_unchecked(366))
            .build()
            .err()
            .expect("Certificate creation must error");
        assert!(matches!(err, CreateCertificateError::CertOutlivesCa { .. }));
    }

    fn assert_certificate_attributes(
        certificate: &TbsCertificateInner,
        subject: &str,
        sans: &[&str],
        san_ips: &[IpAddr],
        validity: Duration,
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

        let san_entries = SubjectAltName::from_der(san_extension.extn_value.as_bytes())
            .expect("failed to parse SAN")
            .0;
        let actual_sans = san_entries
            .iter()
            .filter_map(|san| match san {
                GeneralName::DnsName(dns_name) => Some(dns_name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_sans, sans);
        let actual_san_ips = san_entries
            .iter()
            .filter_map(|san| match san {
                GeneralName::IpAddress(ip) => Some(bytes_to_ip_addr(ip.as_bytes())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_san_ips, san_ips);

        let not_before = certificate.validity.not_before.to_system_time();
        let not_after = certificate.validity.not_after.to_system_time();
        assert_eq!(
            not_after
                .duration_since(not_before)
                .expect("Failed to calculate duration between notBefore and notAfter"),
            *validity
        );
    }

    fn bytes_to_ip_addr(bytes: &[u8]) -> IpAddr {
        match bytes.len() {
            4 => {
                let mut array = [0u8; 4];
                array.copy_from_slice(bytes);
                IpAddr::V4(Ipv4Addr::from(array))
            }
            16 => {
                let mut array = [0u8; 16];
                array.copy_from_slice(bytes);
                IpAddr::V6(Ipv6Addr::from(array))
            }
            _ => panic!(
                "Invalid IP byte length: expected 4 or 16, got {}",
                bytes.len()
            ),
        }
    }
}
