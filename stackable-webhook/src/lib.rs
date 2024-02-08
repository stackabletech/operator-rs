//! Utility types and functions to easily create ready-to-use webhook servers
//! which can handle different tasks, for example CRD conversions. All webhook
//! servers use HTTPS per default and provide options to enable HTTP to HTTPS
//! redirection as well.
//!
//! The crate is also fully compatible with [`tracing`], and emits multiple
//! levels of tracing data.
use axum::Router;
use snafu::Snafu;
use tracing::{debug, warn};

use crate::{
    options::{Options, RedirectOption},
    redirect::Redirector,
    tls::TlsServer,
};

pub mod constants;
pub mod options;
pub mod redirect;
pub mod servers;
pub mod tls;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {}

/// A ready-to-use webhook server.
///
/// This server abstracts away lower-level details like TLS termination
/// and other various configurations, validations or middlewares. The routes
/// and their handlers are completely customizable by bringing your own
/// Axum [`Router`].
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

        // Create server for TLS termination
        // TODO (@Techassi): Remove unwrap
        let tls_server =
            TlsServer::new(self.options.socket_addr, router, self.options.tls).unwrap();
        tls_server.run().await;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::{routing::get, Router};

    #[tokio::test]
    async fn test() {
        let router = Router::new().route("/", get(|| async { "Ok" }));
        let options = Options::builder()
            .tls_mount(
                "/tmp/webhook-certs/serverCert.pem",
                "/tmp/webhook-certs/serverKey.pem",
            )
            .build();

        let server = WebhookServer::new(router, options);
        server.run().await
    }
}
