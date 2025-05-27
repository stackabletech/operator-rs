//! Contains types and functions to generate and sign certificate authorities
//! (CAs) and certificates.
use std::fmt::Debug;

use x509_cert::{Certificate, name::RdnSequence, spki::EncodePublicKey};

use crate::{
    CertificatePair,
    keys::{CertificateKeypair, ecdsa, rsa},
};

mod ca_builder;
mod consts;
mod k8s;
pub use ca_builder::*;
pub use consts::*;
pub use k8s::*;

/// A certificate authority (CA) which is used to generate and sign intermediate or leaf
/// certificates.
///
/// Use [`CertificateAuthorityBuilder`] to create new certificates.
#[derive(Debug)]
pub struct CertificateAuthority<SK>
where
    SK: CertificateKeypair,
    <SK::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate_pair: CertificatePair<SK>,
}

impl<SK> CertificateAuthority<SK>
where
    SK: CertificateKeypair,
    <SK::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    pub fn new(certificate_pair: CertificatePair<SK>) -> Self {
        Self { certificate_pair }
    }

    /// Use this function in combination with [`CertificateAuthorityBuilder`] to create new CAs.
    pub fn builder() -> CertificateAuthorityBuilderBuilder<'static, SK> {
        CertificateAuthorityBuilder::start_builder()
    }

    pub fn signing_key(&self) -> &SK::SigningKey {
        self.certificate_pair.key_pair().signing_key()
    }

    pub fn ca_cert(&self) -> &Certificate {
        &self.certificate_pair.certificate
    }

    pub fn issuer_name(&self) -> &RdnSequence {
        &self.ca_cert().tbs_certificate.issuer
    }
}

impl CertificateAuthority<rsa::SigningKey> {
    /// Same as [`Self::builder`], but enforces the RSA algorithm for key creation.
    pub fn builder_with_rsa() -> CertificateAuthorityBuilderBuilder<'static, rsa::SigningKey> {
        Self::builder()
    }
}

impl CertificateAuthority<ecdsa::SigningKey> {
    /// Same as [`Self::builder`], but enforces the ecdsa algorithm for key creation.
    pub fn builder_with_ecdsa() -> CertificateAuthorityBuilderBuilder<'static, ecdsa::SigningKey> {
        Self::builder()
    }
}
