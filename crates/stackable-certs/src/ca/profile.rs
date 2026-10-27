//! Contains the certificate profiles used to build CA and leaf certificates.
//!
//! x509-cert 0.3 only ships the CA/Browser Forum and IEEE 802.1AR profiles. The CA/Browser Forum
//! profiles are meant for publicly trusted certificates and for example require the CA subject to
//! contain a country and organization name. These requirements don't apply to our internal CAs, so
//! this module provides a port of the `Root` and `Leaf` profiles of x509-cert 0.2 instead. This keeps
//! the generated certificates unchanged.
use x509_cert::{
    TbsCertificate,
    builder::{self, profile::BuilderProfile},
    ext::{
        Extension, ToExtension,
        pkix::{
            AuthorityKeyIdentifier, BasicConstraints, KeyUsage, KeyUsages, SubjectKeyIdentifier,
        },
    },
    name::Name,
    spki::SubjectPublicKeyInfoRef,
};

/// Determines which default extensions are added to a certificate.
pub enum Profile {
    /// Build a self-signed root CA certificate.
    Root {
        /// The subject (and issuer) of the certificate.
        subject: Name,
    },

    /// Build an end entity certificate.
    Leaf {
        /// The name of the CA signing the certificate.
        issuer: Name,

        /// The subject of the certificate.
        subject: Name,

        /// Whether the key agreement flag of the KeyUsage extension should be enabled.
        enable_key_agreement: bool,

        /// Whether the key encipherment flag of the KeyUsage extension should be enabled.
        enable_key_encipherment: bool,
    },
}

impl BuilderProfile for Profile {
    fn get_issuer(&self, subject: &Name) -> Name {
        match self {
            Self::Root { .. } => subject.clone(),
            Self::Leaf { issuer, .. } => issuer.clone(),
        }
    }

    fn get_subject(&self) -> Name {
        match self {
            Self::Root { subject } | Self::Leaf { subject, .. } => subject.clone(),
        }
    }

    fn build_extensions(
        &self,
        spk: SubjectPublicKeyInfoRef<'_>,
        issuer_spk: SubjectPublicKeyInfoRef<'_>,
        tbs: &TbsCertificate,
    ) -> builder::Result<Vec<Extension>> {
        let subject = tbs.subject();
        let mut extensions = Vec::new();

        extensions.push(SubjectKeyIdentifier::try_from(spk)?.to_extension(subject, &extensions)?);

        if let Self::Leaf { .. } = self {
            extensions.push(
                AuthorityKeyIdentifier::try_from(issuer_spk)?.to_extension(subject, &extensions)?,
            );
        }

        let basic_constraints = BasicConstraints {
            ca: matches!(self, Self::Root { .. }),
            path_len_constraint: None,
        };
        extensions.push(basic_constraints.to_extension(subject, &extensions)?);

        let key_usage = match self {
            Self::Root { .. } => KeyUsages::KeyCertSign | KeyUsages::CRLSign,
            Self::Leaf {
                enable_key_agreement,
                enable_key_encipherment,
                ..
            } => {
                let mut key_usage = KeyUsages::DigitalSignature | KeyUsages::NonRepudiation;
                if *enable_key_encipherment {
                    key_usage |= KeyUsages::KeyEncipherment;
                }
                if *enable_key_agreement {
                    key_usage |= KeyUsages::KeyAgreement;
                }
                key_usage
            }
        };
        extensions.push(KeyUsage(key_usage).to_extension(subject, &extensions)?);

        Ok(extensions)
    }
}
