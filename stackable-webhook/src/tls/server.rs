//! This module contains structs and functions to easily create a TLS termination
//! server, which can be used in combination with an Axum [`Router`].
use std::{net::SocketAddr, path::Path, sync::Arc};

use axum::{extract::Request, Router};
use futures_util::pin_mut;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tower::Service;
use tracing::{error, warn};

use crate::CertificateChain;

/// A server which terminates TLS connections and allows clients to commnunicate
/// via HTTPS with the underlying HTTP router.
pub struct TlsServer {
    config: Arc<ServerConfig>,
    socket_addr: SocketAddr,
    router: Router,
}

impl TlsServer {
    pub fn new(
        socket_addr: SocketAddr,
        router: Router,
        cert_path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
    ) -> Self {
        // TODO (@Techassi): Remove unwrap
        let (chain, private_key) = CertificateChain::from_files(cert_path, key_path)
            .unwrap()
            .into_parts();

        // TODO (@Techassi): Use the latest version of rustls related crates
        // TODO (@Techassi): Remove expect
        let mut config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(chain, private_key)
            .expect("bad certificate/key");

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        let config = Arc::new(config);

        Self {
            socket_addr,
            config,
            router,
        }
    }

    pub async fn run(self) {
        // TODO (@Techassi): Remove unwrap
        let tls_acceptor = TlsAcceptor::from(self.config);
        let tcp_listener = TcpListener::bind(self.socket_addr).await.unwrap();

        pin_mut!(tcp_listener);
        loop {
            let tls_acceptor = tls_acceptor.clone();
            let router = self.router.clone();

            // Wait for new tcp connection
            let (tcp_stream, remote_addr) = tcp_listener.accept().await.unwrap();

            tokio::spawn(async move {
                // Wait for tls handshake to happen
                let Ok(tls_stream) = tls_acceptor.accept(tcp_stream).await else {
                    error!("error during tls handshake connection from {}", remote_addr);
                    return;
                };

                // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
                // `TokioIo` converts between them.
                let tls_stream = TokioIo::new(tls_stream);

                // Hyper also has its own `Service` trait and doesn't use tower. We can use
                // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
                // `tower::Service::call`.
                let hyper_service = service_fn(move |request: Request<Incoming>| {
                    // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                    // tower's `Service` requires `&mut self`.
                    //
                    // We don't need to call `poll_ready` since `Router` is always ready.
                    router.clone().call(request)
                });

                if let Err(err) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(tls_stream, hyper_service)
                    .await
                {
                    warn!(%err, "failed to serve connection from {}", remote_addr);
                }
            });
        }
    }
}
