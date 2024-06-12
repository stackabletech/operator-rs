//! This module contains structs and functions to easily create a TLS termination
//! server, which can be used in combination with an Axum [`Router`].
use std::{net::SocketAddr, sync::Arc};

use axum::{extract::Request, Router};
use futures_util::pin_mut;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use snafu::{ResultExt, Snafu};
use stackable_certs::{ca::CertificateAuthority, keys::rsa, CertificatePairError};
use stackable_operator::time::Duration;
use tokio::net::TcpListener;
use tokio_rustls::{
    rustls::{
        crypto::aws_lc_rs::default_provider,
        version::{TLS12, TLS13},
        ServerConfig,
    },
    TlsAcceptor,
};
use tower::Service;
use tracing::instrument;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to construct TLS server config, bad certificate/key"))]
    InvalidTlsPrivateKey { source: tokio_rustls::rustls::Error },

    #[snafu(display(
        "failed to create TCP listener by binding to socket address {socket_addr:?}"
    ))]
    BindTcpListener {
        source: std::io::Error,
        socket_addr: SocketAddr,
    },

    #[snafu(display("failed to create CA to generate and sign webhook leaf certificate"))]
    CreateCertificateAuthority { source: stackable_certs::ca::Error },

    #[snafu(display("failed to generate webhook leaf certificate"))]
    GenerateLeafCertificate { source: stackable_certs::ca::Error },

    #[snafu(display("failed to encode leaf certificate as DER"))]
    EncodeCertificateDer {
        source: CertificatePairError<rsa::Error>,
    },

    #[snafu(display("failed to encode private key as DER"))]
    EncodePrivateKeyDer {
        source: CertificatePairError<rsa::Error>,
    },

    #[snafu(display("failed to set safe TLS protocol versions"))]
    SetSafeTlsProtocolVersions { source: tokio_rustls::rustls::Error },
}

/// Custom implementation of [`std::cmp::PartialEq`] because some inner types
/// don't implement it.
///
/// Note that this implementation is restritced to testing because there are
/// variants that use [`stackable_certs::ca::Error`] which only implements
/// [`PartialEq`] for tests.
#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::BindTcpListener {
                    source: lhs_source,
                    socket_addr: lhs_socket_addr,
                },
                Self::BindTcpListener {
                    source: rhs_source,
                    socket_addr: rhs_socket_addr,
                },
            ) => lhs_socket_addr == rhs_socket_addr && lhs_source.kind() == rhs_source.kind(),
            (lhs, rhs) => lhs == rhs,
        }
    }
}

/// A server which terminates TLS connections and allows clients to commnunicate
/// via HTTPS with the underlying HTTP router.
pub struct TlsServer {
    config: Arc<ServerConfig>,
    socket_addr: SocketAddr,
    router: Router,
}

impl TlsServer {
    #[instrument(name = "create_tls_server", skip(router))]
    pub async fn new(socket_addr: SocketAddr, router: Router) -> Result<Self> {
        let mut certificate_authority =
            CertificateAuthority::new_rsa().context(CreateCertificateAuthoritySnafu)?;

        let leaf_certificate = certificate_authority
            .generate_rsa_leaf_certificate("Leaf", "webhook", Duration::from_secs(3600))
            .context(GenerateLeafCertificateSnafu)?;

        let certificate_der = leaf_certificate
            .certificate_der()
            .context(EncodeCertificateDerSnafu)?;

        let private_key_der = leaf_certificate
            .private_key_der()
            .context(EncodePrivateKeyDerSnafu)?;

        let tls_provider = default_provider();
        let mut config = ServerConfig::builder_with_provider(tls_provider.into())
            .with_protocol_versions(&[&TLS12, &TLS13])
            .context(SetSafeTlsProtocolVersionsSnafu)?
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], private_key_der)
            .context(InvalidTlsPrivateKeySnafu)?;

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        let config = Arc::new(config);

        Ok(Self {
            socket_addr,
            config,
            router,
        })
    }

    /// Runs the TLS server by listening for incoming TCP connections on the
    /// bound socket address. It only accepts TLS connections. Internally each
    /// TLS stream get handled by a Hyper service, which in turn is an Axum
    /// router.
    pub async fn run(self) -> Result<()> {
        let tls_acceptor = TlsAcceptor::from(self.config);
        let tcp_listener =
            TcpListener::bind(self.socket_addr)
                .await
                .context(BindTcpListenerSnafu {
                    socket_addr: self.socket_addr,
                })?;

        pin_mut!(tcp_listener);
        loop {
            let tls_acceptor = tls_acceptor.clone();
            let router = self.router.clone();

            // Wait for new tcp connection
            let (tcp_stream, remote_addr) = match tcp_listener.accept().await {
                Ok((stream, addr)) => (stream, addr),
                Err(err) => {
                    tracing::warn!(%err, "failed to accept incoming TCP connection");
                    continue;
                }
            };

            tokio::spawn(async move {
                // Wait for tls handshake to happen
                let Ok(tls_stream) = tls_acceptor.accept(tcp_stream).await else {
                    tracing::trace!(%remote_addr, "error during tls handshake connection");
                    return;
                };

                // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
                // `TokioIo` converts between them.
                let tls_stream = TokioIo::new(tls_stream);

                // Hyper also has its own `Service` trait and doesn't use tower. We can use
                // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
                // `tower::Service::call`.
                let service = service_fn(move |request: Request<Incoming>| {
                    // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                    // tower's `Service` requires `&mut self`.
                    //
                    // We don't need to call `poll_ready` since `Router` is always ready.
                    router.clone().call(request)
                });

                if let Err(err) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(tls_stream, service)
                    .await
                {
                    tracing::warn!(%err, %remote_addr, "failed to serve connection");
                }
            });
        }
    }
}
