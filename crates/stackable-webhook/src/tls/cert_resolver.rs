use std::sync::Arc;

use arc_swap::ArcSwap;
use snafu::{OptionExt, ResultExt, Snafu};
use stackable_certs::{CertificatePairError, ca::CertificateAuthority, keys::ecdsa};
use tokio::sync::mpsc;
use tokio_rustls::rustls::{
    crypto::CryptoProvider, server::ResolvesServerCert, sign::CertifiedKey,
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

    #[snafu(display("failed to create packaged certificate chain from DER"))]
    DecodeCertifiedKeyFromDer { source: tokio_rustls::rustls::Error },

    #[snafu(display("failed to run task in blocking thread"))]
    TokioSpawnBlocking { source: tokio::task::JoinError },

    #[snafu(display("no default rustls CryptoProvider installed"))]
    NoDefaultCryptoProviderInstalled,
}

/// This struct serves as [`ResolvesServerCert`] to always hand out the current certificate for TLS
/// client connections.
///
/// It offers the [`Self::rotate_certificate`] function to create a fresh certificate and basically
/// hot-reload the certificate in the running webhook.
#[derive(Debug)]
pub struct CertificateResolver {
    /// Using a [`ArcSwap`] (over e.g. [`tokio::sync::RwLock`]), so that we can easily
    /// (and performant) bridge between async write and sync read.
    current_certified_key: ArcSwap<CertifiedKey>,
    subject_alterative_dns_names: Arc<Vec<String>>,

    certificate_tx: mpsc::Sender<Certificate>,
}

impl CertificateResolver {
    pub async fn new(
        subject_alterative_dns_names: Vec<String>,
        certificate_tx: mpsc::Sender<Certificate>,
    ) -> Result<Self> {
        let subject_alterative_dns_names = Arc::new(subject_alterative_dns_names);
        let certified_key = Self::generate_new_certificate_inner(
            subject_alterative_dns_names.clone(),
            &certificate_tx,
        )
        .await?;

        Ok(Self {
            subject_alterative_dns_names,
            current_certified_key: ArcSwap::new(certified_key),
            certificate_tx,
        })
    }

    pub async fn rotate_certificate(&self) -> Result<()> {
        let certified_key = self.generate_new_certificate().await?;

        // TODO: Sign the new cert somehow with the old cert. See https://github.com/stackabletech/decisions/issues/56
        self.current_certified_key.store(certified_key);

        Ok(())
    }

    async fn generate_new_certificate(&self) -> Result<Arc<CertifiedKey>> {
        let subject_alterative_dns_names = self.subject_alterative_dns_names.clone();
        Self::generate_new_certificate_inner(subject_alterative_dns_names, &self.certificate_tx)
            .await
    }

    /// Creates a new certificate and returns the certified key.
    ///
    /// The certificate is send to the passed `cert_tx`.
    ///
    /// FIXME: This should *not* construct a CA cert and cert, but only a cert!
    /// This needs some changes in stackable-certs though.
    /// See [the relevant decision](https://github.com/stackabletech/decisions/issues/56)
    async fn generate_new_certificate_inner(
        subject_alterative_dns_names: Arc<Vec<String>>,
        certificate_tx: &mpsc::Sender<Certificate>,
    ) -> Result<Arc<CertifiedKey>> {
        // The certificate generations can take a while, so we use `spawn_blocking`
        let (cert, certified_key) = tokio::task::spawn_blocking(move || {
            let tls_provider =
                CryptoProvider::get_default().context(NoDefaultCryptoProviderInstalledSnafu)?;

            let ca_key = ecdsa::SigningKey::new().context(GenerateEcdsaSigningKeySnafu)?;
            let mut ca =
                CertificateAuthority::new_with(ca_key, rand::random::<u64>(), WEBHOOK_CA_LIFETIME)
                    .context(CreateCertificateAuthoritySnafu)?;

            let certificate_pair = ca
                .generate_ecdsa_leaf_certificate(
                    "Leaf",
                    "webhook",
                    subject_alterative_dns_names.iter().map(|san| san.as_str()),
                    WEBHOOK_CERTIFICATE_LIFETIME,
                )
                .context(GenerateLeafCertificateSnafu)?;

            let certificate_der = certificate_pair
                .certificate_der()
                .context(EncodeCertificateDerSnafu)?;
            let private_key_der = certificate_pair
                .private_key_der()
                .context(EncodePrivateKeyDerSnafu)?;
            let certificate_key =
                CertifiedKey::from_der(vec![certificate_der], private_key_der, tls_provider)
                    .context(DecodeCertifiedKeyFromDerSnafu)?;

            Ok((
                certificate_pair.certificate().clone(),
                Arc::new(certificate_key),
            ))
        })
        .await
        .context(TokioSpawnBlockingSnafu)??;

        certificate_tx
            .send(cert)
            .await
            .map_err(|_err| CertificateResolverError::SendCertificateToChannel)?;

        Ok(certified_key)
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
