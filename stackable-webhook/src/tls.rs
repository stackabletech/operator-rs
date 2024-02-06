use std::{fs::File, io::BufReader, net::SocketAddr, path::Path, sync::Arc};

use axum::{extract::Request, Router};
use futures_util::pin_mut;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls_pemfile::{certs, pkcs8_private_keys};
use tokio::net::TcpListener;
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};
use tower::Service;
use tracing::{error, warn};

pub struct TlsServer {
    config: Arc<ServerConfig>,
    socket_addr: SocketAddr,
    router: Router,
}

impl TlsServer {
    pub fn new(
        socket_addr: SocketAddr,
        router: Router,
        cert_file: impl AsRef<Path>,
        key_file: impl AsRef<Path>,
    ) -> Self {
        // TODO (@Techassi): Abstract away the cert chain loading
        // TODO (@Techassi): Remove unwraps
        let mut cert_file = &mut BufReader::new(File::open(cert_file).unwrap());
        let mut key_file = &mut BufReader::new(File::open(key_file).unwrap());

        // TODO (@Techassi): Remove unwrap
        let key = PrivateKey(pkcs8_private_keys(&mut key_file).unwrap().remove(0));
        let certs = certs(&mut cert_file)
            .unwrap()
            .into_iter()
            .map(Certificate)
            .collect();

        // TODO (@Techassi): Use the latest version of rustls related crates
        // TODO (@Techassi): Remove expect
        let mut config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
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
                let hyper_service =
                    hyper::service::service_fn(move |request: Request<Incoming>| {
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
