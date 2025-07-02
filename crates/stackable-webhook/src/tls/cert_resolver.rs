use std::sync::Arc;

use arc_swap::ArcSwap;
use snafu::{ResultExt, Snafu};
use stackable_certs::{CertificatePairError, ca::CertificateAuthority, keys::ecdsa};
use tokio::sync::mpsc;
use tokio_rustls::rustls::{
    crypto::ring::default_provider, server::ResolvesServerCert, sign::CertifiedKey,
};
use x509_cert::Certificate;

use super::{WEBHOOK_CA_LIFETIME, WEBHOOK_CERTIFICATE_LIFETIME};

type Result<T, E = CertificateResolverError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum CertificateResolverError {
    #[snafu(display("failed send certificate to channel"))]
    SendCertificateToChannel,

    #[snafu(display("failed to generate ECDSA signing key"))]
    GenerateEcdsaSigningKey { source: ecdsa::Error },

    #[snafu(display("failed to generate new certificate"))]
    GenerateNewCertificate {
        #[snafu(source(from(CertificateResolverError, Box::new)))]
        source: Box<CertificateResolverError>,
    },

    #[snafu(display("failed to create CA to generate and sign webhook leaf certificate"))]
    CreateCertificateAuthority { source: stackable_certs::ca::Error },

    #[snafu(display("failed to generate webhook leaf certificate"))]
    GenerateLeafCertificate { source: stackable_certs::ca::Error },

    #[snafu(display("failed to encode leaf certificate as DER"))]
    EncodeCertificateDer {
        source: CertificatePairError<ecdsa::Error>,
    },

    #[snafu(display("failed to encode private key as DER"))]
    EncodePrivateKeyDer {
        source: CertificatePairError<ecdsa::Error>,
    },

    #[snafu(display("failed to decode CertifiedKey from DER"))]
    DecodeCertifiedKeyFromDer { source: tokio_rustls::rustls::Error },

    #[snafu(display("failed to run task in blocking thread"))]
    TokioSpawnBlocking { source: tokio::task::JoinError },
}

/// This struct serves as [`ResolvesServerCert`] to always hand out the current certificate for TLS
/// client connections.
///
/// It offers the [`Self::rotate_certificate`] function to create a fresh certificate and basically
/// hot-reload the certificate in the running webhook.
#[derive(Debug)]
pub struct CertificateResolver {
    /// Using a [`ArcSwap`] (over e.g. [`tokio::sync::RwLock`]), so that we can easily
    /// (and performant) bridge between async write and sync write.
    current_certified_key: ArcSwap<CertifiedKey>,
    subject_alterative_dns_names: Arc<Vec<String>>,

    cert_tx: mpsc::Sender<Certificate>,
}

impl CertificateResolver {
    pub async fn new(
        subject_alterative_dns_names: Vec<String>,
        cert_tx: mpsc::Sender<Certificate>,
    ) -> Result<Self> {
        let subject_alterative_dns_names = Arc::new(subject_alterative_dns_names);
        let (cert, certified_key) = Self::generate_new_cert(subject_alterative_dns_names.clone())
            .await
            .context(GenerateNewCertificateSnafu)?;

        cert_tx
            .send(cert)
            .await
            .map_err(|_err| CertificateResolverError::SendCertificateToChannel)?;

        Ok(Self {
            subject_alterative_dns_names,
            current_certified_key: ArcSwap::new(certified_key),
            cert_tx,
        })
    }

    pub async fn rotate_certificate(&self) -> Result<()> {
        let (cert, certified_key) =
            Self::generate_new_cert(self.subject_alterative_dns_names.clone())
                .await
                .context(GenerateNewCertificateSnafu)?;

        // TODO: Sign the new cert somehow with the old cert. See https://github.com/stackabletech/decisions/issues/56

        self.cert_tx
            .send(cert)
            .await
            .map_err(|_err| CertificateResolverError::SendCertificateToChannel)?;

        self.current_certified_key.store(certified_key);

        Ok(())
    }

    /// FIXME: This should *not* construct a CA cert and cert, but only a cert!
    /// This needs some changes in stackable-certs though.
    /// See https://github.com/stackabletech/decisions/issues/56
    async fn generate_new_cert(
        subject_alterative_dns_names: Arc<Vec<String>>,
    ) -> Result<(Certificate, Arc<CertifiedKey>)> {
        // The certificate generations can take a while, so we use `spawn_blocking`
        tokio::task::spawn_blocking(move || {
            let tls_provider = default_provider();

            let ca_key = ecdsa::SigningKey::new().context(GenerateEcdsaSigningKeySnafu)?;
            let mut ca =
                CertificateAuthority::new_with(ca_key, rand::random::<u64>(), WEBHOOK_CA_LIFETIME)
                    .context(CreateCertificateAuthoritySnafu)?;

            let certificate = ca
                .generate_ecdsa_leaf_certificate(
                    "Leaf",
                    "webhook",
                    subject_alterative_dns_names.iter().map(|san| san.as_str()),
                    WEBHOOK_CERTIFICATE_LIFETIME,
                )
                .context(GenerateLeafCertificateSnafu)?;

            let certificate_der = certificate
                .certificate_der()
                .context(EncodeCertificateDerSnafu)?;
            let private_key_der = certificate
                .private_key_der()
                .context(EncodePrivateKeyDerSnafu)?;
            let certificate_key =
                CertifiedKey::from_der(vec![certificate_der], private_key_der, &tls_provider)
                    .context(DecodeCertifiedKeyFromDerSnafu)?;

            Ok((certificate.certificate().clone(), Arc::new(certificate_key)))
        })
        .await
        .context(TokioSpawnBlockingSnafu)?
    }
}

impl ResolvesServerCert for CertificateResolver {
    fn resolve(
        &self,
        _client_hello: tokio_rustls::rustls::server::ClientHello<'_>,
    ) -> Option<Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        Some(self.current_certified_key.load().clone())
    }
}
