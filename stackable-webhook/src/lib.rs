//! Utility types and functions to easily create ready-to-use webhook servers
//! which can handle different tasks, for example CRD conversions. All webhook
//! servers use HTTPS per default and provide options to enable HTTP to HTTPS
//! redirection as well.
//!
//! The crate is also fully compatible with [`tracing`], and emits multiple
//! levels of tracing data.

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tokio_rustls::rustls::ServerConfig;
use tracing::{debug, warn};

pub mod constants;
pub mod conversion;
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

        // Create the root router and merge the provided router into it.
        let mut router = Router::new();
        router = router.merge(self.router);

        let listener = TcpListener::bind(self.options.socket_addr).await.unwrap();
        axum::serve(listener, router).await.unwrap()
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
