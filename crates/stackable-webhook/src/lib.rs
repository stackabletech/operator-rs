//! Utility types and functions to easily create ready-to-use webhook servers
//! which can handle different tasks, for example CRD conversions. All webhook
//! servers use HTTPS by defaultThis library is fully compatible with the
//! [`tracing`] crate and emits debug level tracing data.
//!
//! Most users will only use the top-level exported generic [`WebhookServer`]
//! which enables complete control over the [Router] which handles registering
//! routes and their handler functions.
//!
//! ```
//! use stackable_webhook::{WebhookServer, Options};
//! use axum::Router;
//!
//! let router = Router::new();
//! let server = WebhookServer::new(router, Options::default());
//! ```
//!
//! For some usages, complete end-to-end [`WebhookServer`] implementations
//! exist. One such implementation is the [`ConversionWebhookServer`][1]. The
//! only required parameters are a conversion handler function and [`Options`].
//!
//! This library additionally also exposes lower-level structs and functions to
//! enable complete controll over these details if needed.
//!
//! [1]: crate::servers::ConversionWebhookServer
use axum::{body::Body, http::Request, Router};
use snafu::{ResultExt, Snafu};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, debug_span, instrument};

use crate::tls::TlsServer;

pub mod constants;
pub mod options;
pub mod servers;
pub mod tls;

// Selected re-exports
pub use crate::options::Options;

/// A result type alias with the library-level [`Error`] type as teh default
/// error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A generic webhook handler receiving a request and sending back a response.
///
/// This trait is not intended to be implemented by external crates and this
/// library provides various ready-to-use implementations for it. One such an
/// implementation is part of the [`ConversionWebhookServer`][1].
///
/// [1]: crate::servers::ConversionWebhookServer
pub(crate) trait WebhookHandler<Req, Res> {
    fn call(self, req: Req) -> Res;
}

/// A generic webhook handler receiving a request and state and sending back
/// a response.
///
/// This trait is not intended to be  implemented by external crates and this
/// library provides various ready-to-use implementations for it. One such an
/// implementation is part of the [`ConversionWebhookServer`][1].
///
/// [1]: crate::servers::ConversionWebhookServer
pub(crate) trait StatefulWebhookHandler<Req, Res, S> {
    fn call(self, req: Req, state: S) -> Res;
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create TLS server"))]
    CreateTlsServer { source: tls::Error },

    #[snafu(display("failed to run TLS server"))]
    RunTlsServer { source: tls::Error },
}

/// A ready-to-use webhook server.
///
/// This server abstracts away lower-level details like TLS termination
/// and other various configurations, validations or middlewares. The routes
/// and their handlers are completely customizable by bringing your own
/// Axum [`Router`].
///
/// For complete end-to-end implementations, see [`ConversionWebhookServer`][1].
///
/// [1]: crate::servers::ConversionWebhookServer
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
    ///     .bind_address([127, 0, 0, 1], 8080)
    ///     .build();
    ///
    /// let router = Router::new();
    /// let server = WebhookServer::new(router, options);
    /// ```
    #[instrument(name = "create_webhook_server", skip(router))]
    pub fn new(router: Router, options: Options) -> Self {
        debug!("create new webhook server");
        Self { options, router }
    }

    /// Runs the webhook server by creating a TCP listener and binding it to
    /// the specified socket address.
    #[instrument(name = "run_webhook_server", skip(self), fields(self.options))]
    pub async fn run(self) -> Result<()> {
        debug!("run webhook server");

        // TODO (@Techassi): Switch out for Otel compatible tracing
        // https://github.com/davidB/tracing-opentelemetry-instrumentation-sdk

        // Create a high-level tracing layer
        debug!("create tracing service (layer)");
        let layer = TraceLayer::new_for_http()
            .make_span_with(|_: &Request<Body>| debug_span!("webhook_request"))
            .on_body_chunk(())
            .on_eos(());

        let service = ServiceBuilder::new().layer(layer);

        // Create the root router and merge the provided router into it.
        debug!("create core router and merge provided router");
        let mut router = Router::new().layer(service);
        router = router.merge(self.router);

        // Create server for TLS termination
        debug!("create TLS server");
        let tls_server = TlsServer::new(self.options.socket_addr, router)
            .await
            .context(CreateTlsServerSnafu)?;

        debug!("running TLS server");
        tls_server.run().await.context(RunTlsServerSnafu)
    }
}
