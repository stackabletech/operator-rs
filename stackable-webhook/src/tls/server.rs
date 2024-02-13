//! This module contains structs and functions to easily create a TLS termination
//! server, which can be used in combination with an Axum [`Router`].
use std::{net::SocketAddr, sync::Arc};

use axum::{extract::Request, Router};
use futures_util::pin_mut;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use snafu::{ResultExt, Snafu};
use tokio::net::TcpListener;
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tower::Service;
use tracing::{error, instrument, warn};

use crate::{
    options::TlsOption,
    tls::certs::{CertifacteError, CertificateChain},
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create TLS certificate chain"))]
    TlsCertificateChain { source: CertifacteError },

    #[snafu(display("failed to construct TLS server config, bad certificate/key"))]
    InvalidTlsPrivateKey { source: tokio_rustls::rustls::Error },

    #[snafu(display(
        "failed to create TCP listener by binding to socket address {socket_addr:?}"
    ))]
    BindTcpListener {
        source: std::io::Error,
        socket_addr: SocketAddr,
    },
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
    pub fn new(socket_addr: SocketAddr, router: Router, tls: TlsOption) -> Result<Self> {
        let config = match tls {
            TlsOption::AutoGenerate => {
                // let mut config = ServerConfig::builder()
                //     .with_safe_defaults()
                //     .with_no_client_auth()
                //     .with_cert_resolver(cert_resolver);
                // config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
                todo!()
            }
            TlsOption::Mount {
                cert_path,
                pk_path,
                pk_encoding,
            } => {
                let (chain, private_key) =
                    CertificateChain::from_files(cert_path, pk_path, pk_encoding)
                        .context(TlsCertificateChainSnafu)?
                        .into_parts();

                // TODO (@Techassi): Use the latest version of rustls related crates
                let mut config = ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(chain, private_key)
                    .context(InvalidTlsPrivateKeySnafu)?;

                config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
                config
            }
        };
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
    #[instrument(name = "run_tls_server", skip(self), fields(self.socket_addr, self.config))]
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
                    warn!(%err, "failed to accept incoming TCP connection");
                    continue;
                }
            };

            tokio::spawn(async move {
                // Wait for tls handshake to happen
                let Ok(tls_stream) = tls_acceptor.accept(tcp_stream).await else {
                    error!(%remote_addr, "error during tls handshake connection");
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
                    warn!(%err, %remote_addr, "failed to serve connection");
                }
            });
        }
    }
}
