use bon::Builder;
use rsa::pkcs8::EncodePublicKey;
use snafu::{ResultExt, Snafu};
use stackable_operator::time::Duration;
use tracing::{debug, instrument};
use x509_cert::{
    builder::{Builder, CertificateBuilder, Profile},
    der::{DecodePem, referenced::OwnedToRef},
    ext::pkix::AuthorityKeyIdentifier,
    name::Name,
    serial_number::SerialNumber,
    spki::SubjectPublicKeyInfoOwned,
    time::Validity,
};

use super::CertificateAuthority;
use crate::{
    CertificatePair,
    ca::{DEFAULT_CA_VALIDITY, PEM_LINE_ENDING, SDP_ROOT_CA_SUBJECT},
    keys::CertificateKeypair,
};

/// Defines all error variants which can occur when creating a CA
#[derive(Debug, Snafu)]
pub enum CreateCertificateAuthorityError<E>
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

    #[snafu(display("failed to create signing key pair"))]
    CreateSigningKeyPair { source: E },

    #[snafu(display("failed to serialize public key as PEM"))]
    SerializePublicKey { source: x509_cert::spki::Error },

    #[snafu(display("failed to decode SPKI from PEM"))]
    DecodeSpkiFromPem { source: x509_cert::der::Error },

    #[snafu(display("failed to parse AuthorityKeyIdentifier"))]
    ParseAuthorityKeyIdentifier { source: x509_cert::der::Error },

    #[snafu(display("failed to create certificate builder"))]
    CreateCertificateBuilder { source: x509_cert::builder::Error },

    #[snafu(display("failed to add certificate extension"))]
    AddCertificateExtension { source: x509_cert::builder::Error },

    #[snafu(display("failed to build certificate"))]
    BuildCertificate { source: x509_cert::builder::Error },
}

/// This builder builds certificate authorities of type [`CertificateAuthority`].
///
/// It has many default values, notably;
///
/// - A default validity of [`DEFAULT_CA_VALIDITY`]
/// - A default subject of [`SDP_ROOT_CA_SUBJECT`]
/// - A randomly generated serial number
/// - In case no `signing_key_pair` was provided, a fresh keypair will be created. The algorithm
///   (`rsa`/`ecdsa`) is chosen by the generic [`CertificateKeypair`] type of this struct.
///
/// The CA contains the public half of the provided `signing_key_pair` and is signed by the private
/// half of said key.
///
/// Example code to construct a CA:
///
/// ```no_run
/// use stackable_certs::{
///     keys::ecdsa, ca::CertificateAuthority,
/// };
///
/// let ca = CertificateAuthority::<ecdsa::SigningKey>::builder()
///     .build()
///     .expect("failed to build CA");
/// ```
///
/// Instead of using generics to determine the algorithm to use you can also use [`CertificateAuthority::builder_with_rsa`]
/// or [`CertificateAuthority::builder_with_ecdsa`] instead:
///
/// ```no_run
/// use stackable_certs::{
///     keys::ecdsa, ca::CertificateAuthority,
/// };
///
/// let ca = CertificateAuthority::builder_with_ecdsa()
///     .build()
///     .expect("failed to build CA");
/// ```
#[derive(Builder)]
#[builder(start_fn = start_builder, finish_fn = finish_builder)]
pub struct CertificateAuthorityBuilder<'a, SKP>
where
    SKP: CertificateKeypair,
    <SKP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    /// Required subject of the certificate authority, usually starts with `CN=`.
    #[builder(default = SDP_ROOT_CA_SUBJECT)]
    subject: &'a str,

    /// Validity/lifetime of the certificate.
    ///
    /// If not specified the default of [`DEFAULT_CA_VALIDITY`] will be used.
    #[builder(default = DEFAULT_CA_VALIDITY)]
    validity: Duration,

    /// Cryptographic keypair used to sign leaf certificates.
    ///
    /// If not specified a random keypair will be generated.
    signing_key_pair: Option<SKP>,
}

impl<SKP, S> CertificateAuthorityBuilderBuilder<'_, SKP, S>
where
    SKP: CertificateKeypair,
    <SKP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
    S: certificate_authority_builder_builder::IsComplete,
{
    /// Convenience function to avoid calling `builder().finish_builder().build()`
    pub fn build(
        self,
    ) -> Result<CertificateAuthority<SKP>, CreateCertificateAuthorityError<SKP::Error>> {
        self.finish_builder().build()
    }
}

impl<SKP> CertificateAuthorityBuilder<'_, SKP>
where
    SKP: CertificateKeypair,
    <SKP::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    #[instrument(
        name = "build_certificate_authority",
        skip(self),
        fields(subject = self.subject),
    )]
    pub fn build(
        self,
    ) -> Result<CertificateAuthority<SKP>, CreateCertificateAuthorityError<SKP::Error>> {
        let validity = Validity::from_now(*self.validity).context(ParseValiditySnafu)?;
        let subject: Name = self.subject.parse().context(ParseSubjectSnafu {
            subject: self.subject,
        })?;
        let signing_key_pair = match self.signing_key_pair {
            Some(signing_key_pair) => signing_key_pair,
            None => SKP::new().context(CreateSigningKeyPairSnafu)?,
        };

        // By choosing a random serial number we can make the reasonable assumption that we generate
        // a unique serial for each CA.
        let serial_number = SerialNumber::from(rand::random::<u64>());

        let spki_pem = signing_key_pair
            .verifying_key()
            .to_public_key_pem(PEM_LINE_ENDING)
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
        // subject public key. This conforms to one of the outlined methods for
        // generating key identifiers outlined in RFC 5280, section 4.2.1.2.
        //
        // Prepare extensions so we can avoid clones.
        let aki = AuthorityKeyIdentifier::try_from(spki.owned_to_ref())
            .context(ParseAuthorityKeyIdentifierSnafu)?;

        debug!(
            ca.subject = %subject,
            ca.not_after = %validity.not_after,
            ca.not_before = %validity.not_before,
            ca.serial = ?serial_number,
            ca.public_key.algorithm = SKP::algorithm_name(),
            ca.public_key.size = SKP::key_size(),
            "creating certificate authority"
        );
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

        builder
            .add_extension(&aki)
            .context(AddCertificateExtensionSnafu)?;
        let certificate = builder.build().context(BuildCertificateSnafu)?;

        Ok(CertificateAuthority {
            certificate_pair: CertificatePair {
                certificate,
                key_pair: signing_key_pair,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use x509_cert::certificate::TbsCertificateInner;

    use super::*;
    use crate::keys::rsa;

    #[test]
    fn minimal_ca() {
        let ca = CertificateAuthority::builder_with_ecdsa()
            .build()
            .expect("failed to build CA");

        assert_ca_cert_attributes(
            &ca.ca_cert().tbs_certificate,
            SDP_ROOT_CA_SUBJECT,
            DEFAULT_CA_VALIDITY,
        )
    }

    #[test]
    fn customized_ca() {
        let ca = CertificateAuthority::builder()
            .subject("CN=Test")
            .signing_key_pair(rsa::SigningKey::new().unwrap())
            .validity(Duration::from_days_unchecked(13))
            .build()
            .expect("failed to build CA");

        assert_ca_cert_attributes(
            &ca.ca_cert().tbs_certificate,
            "CN=Test",
            Duration::from_days_unchecked(13),
        )
    }

    fn assert_ca_cert_attributes(ca_cert: &TbsCertificateInner, subject: &str, validity: Duration) {
        assert_eq!(ca_cert.subject, subject.parse().unwrap());

        let not_before = ca_cert.validity.not_before.to_system_time();
        let not_after = ca_cert.validity.not_after.to_system_time();
        assert_eq!(
            not_after
                .duration_since(not_before)
                .expect("Failed to calculate duration between notBefore and notAfter"),
            *validity
        );
    }
}
