//! Utility types and functions to easily create ready-to-use webhook servers
//! which can handle different tasks, for example CRD conversions. All webhook
//! servers use HTTPS per default and provide options to enable HTTP to HTTPS
//! redirection as well.
//!
//! The crate is also fully compatible with [`tracing`], and emits multiple
//! levels of tracing data.

use std::{fs::File, io::BufReader, sync::Arc};

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
use tracing::{debug, error, warn};

pub mod constants;
mod options;
mod redirect;

pub use options::*;
pub use redirect::*;

/// A ready-to-use webhook server.
pub struct WebhookServer {
    options: Options,
    router: Router,
}

impl WebhookServer {
    /// Creates a new ready-to-use webhook server.
    ///
    /// The server listens on `socket_addr` which is provided via the [`Options`]
    /// and handles routing based on the provided Axum `router`. Most of the time
    /// it is sufficient to use [`Options::default()`]. See the documentation
    /// for [`Options`] for more details on the default values.
    ///
    /// To start the server, use the [`WebhookServer::run()`] function. This will
    /// run the server using the Tokio runtime until it is terminated.
    ///
    /// ### Basic Example
    ///
    /// ```
    /// use stackable_webhook::{WebhookServer, Options};
    /// use axum::Router;
    ///
    /// let router = Router::new();
    /// let server = WebhookServer::new(router, Options::default());
    /// ```
    ///
    /// ### Example with Custom Options
    ///
    /// ```
    /// use stackable_webhook::{WebhookServer, Options};
    /// use axum::Router;
    ///
    /// let options = Options::builder()
    ///     .disable_redirect()
    ///     .socket_addr(([127, 0, 0, 1], 8080))
    ///     .build();
    ///
    /// let router = Router::new();
    /// let server = WebhookServer::new(router, options);
    /// ```
    pub fn new(router: Router, options: Options) -> Self {
        debug!("create new webhook server");
        Self { options, router }
    }

    /// Runs the webhook server by creating a TCP listener and binding it to
    /// the specified socket address.
    pub async fn run(self) {
        debug!("run webhook server");

        // Only run the auto redirector when enabled
        match self.options.redirect {
            RedirectOption::Enabled(http_port) => {
                debug!("run webhook server with automatic HTTP to HTTPS redirect enabled");

                let redirector = Redirector::new(
                    self.options.socket_addr.ip(),
                    self.options.socket_addr.port(),
                    http_port,
                );

                tokio::spawn(redirector.run());
            }
            RedirectOption::Disabled => {
                warn!("webhook runs without automatic HTTP to HTTPS redirect which is not recommended");
            }
        }

        let mut cert_file = &mut BufReader::new(
            File::open("/apiserver.local.config/certificates/apiserver.crt").unwrap(),
        );
        let mut key_file = &mut BufReader::new(
            File::open("/apiserver.local.config/certificates/apiserver.key").unwrap(),
        );

        let key = PrivateKey(pkcs8_private_keys(&mut key_file).unwrap().remove(0));
        let certs = certs(&mut cert_file)
            .unwrap()
            .into_iter()
            .map(Certificate)
            .collect();

        let mut config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("bad certificate/key");

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let config = Arc::new(config);
        let tls_acceptor = TlsAcceptor::from(config);

        // Create the root router and merge the provided router into it.
        let mut app = Router::new();
        app = app.merge(self.router);

        let tcp_listener = TcpListener::bind(self.options.socket_addr).await.unwrap();
        println!("Binded");

        pin_mut!(tcp_listener);
        loop {
            let tower_service = app.clone();
            let tls_acceptor = tls_acceptor.clone();

            // Wait for new tcp connection
            let (cnx, addr) = tcp_listener.accept().await.unwrap();
            println!("TCP Accepted");

            tokio::spawn(async move {
                // Wait for tls handshake to happen
                let Ok(stream) = tls_acceptor.accept(cnx).await else {
                    error!("error during tls handshake connection from {}", addr);
                    return;
                };

                println!("TLS Accepted");

                // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
                // `TokioIo` converts between them.
                let stream = TokioIo::new(stream);

                // Hyper also has its own `Service` trait and doesn't use tower. We can use
                // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
                // `tower::Service::call`.
                let hyper_service =
                    hyper::service::service_fn(move |request: Request<Incoming>| {
                        // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                        // tower's `Service` requires `&mut self`.
                        //
                        // We don't need to call `poll_ready` since `Router` is always ready.
                        tower_service.clone().call(request)
                    });

                let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(stream, hyper_service)
                    .await;

                if let Err(err) = ret {
                    warn!("error serving connection from {}: {}", addr, err);
                }
            });
        }

        // axum::serve(listener, router).await.unwrap()
    }
}

pub struct TlsServer {
    config: Arc<ServerConfig>,
}

impl TlsServer {
    // pub fn new() -> Self {
    //     let config = ServerConfig::builder()
    //         .with_no_client_auth()
    //         .with_cert_resolver(cert_resolver);
    //     let config = Arc::new(config);

    //     Self { config }
    // }
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::{routing::post, Router};

    #[tokio::test]
    async fn test() {
        let router = Router::new().route("/", post(handler));
        let options = Options::builder()
            .disable_redirect()
            .socket_addr(([127, 0, 0, 1], 8080))
            .build();

        let server = WebhookServer::new(router, options);
        server.run().await
    }

    async fn handler() -> &'static str {
        println!("Test");
        "Ok"
    }
}
