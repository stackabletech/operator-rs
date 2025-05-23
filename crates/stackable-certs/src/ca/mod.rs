//! Contains types and functions to generate and sign certificate authorities
//! (CAs) and certificates.
use std::fmt::Debug;

use x509_cert::{Certificate, name::RdnSequence, spki::EncodePublicKey};

use crate::{CertificatePair, keys::CertificateKeypair};

mod ca_builder;
mod consts;
mod k8s;
pub use ca_builder::*;
pub use consts::*;
pub use k8s::*;

/// A certificate authority (CA) which is used to generate and sign
/// intermediate or leaf certificates.
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

    pub fn builder() -> CertificateAuthorityBuilderBuilder<'static, SK> {
        CertificateAuthorityBuilder::builder()
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
