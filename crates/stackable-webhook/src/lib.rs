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
use axum::{routing::get, Router};
use futures_util::{pin_mut, select, FutureExt as _};
use snafu::{ResultExt, Snafu};
use stackable_telemetry::AxumTraceLayer;
use tokio::signal::unix::{signal, SignalKind};
use tower::ServiceBuilder;

// use tower_http::trace::TraceLayer;
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
    pub fn new(router: Router, options: Options) -> Self {
        tracing::trace!("create new webhook server");
        Self { options, router }
    }

    /// Runs the Webhook server and sets up signal handlers for shutting down.
    ///
    /// This does not implement graceful shutdown of the underlying server.
    pub async fn run(self) -> Result<()> {
        let future_server = self.run_server();
        let future_signal = async {
            let mut sigint = signal(SignalKind::interrupt()).expect("create SIGINT listener");
            let mut sigterm = signal(SignalKind::terminate()).expect("create SIGTERM listener");

            tracing::debug!("created unix signal handlers");

            select! {
                signal = sigint.recv().fuse() => {
                    if signal.is_some() {
                        tracing::debug!( "received SIGINT");
                    }
                },
                signal = sigterm.recv().fuse() => {
                    if signal.is_some() {
                        tracing::debug!( "received SIGTERM");
                    }
                },
            };
        };

        // select requires Future + Unpin
        pin_mut!(future_server);
        pin_mut!(future_signal);

        futures_util::future::select(future_server, future_signal).await;

        Ok(())
    }

    /// Runs the webhook server by creating a TCP listener and binding it to
    /// the specified socket address.
    async fn run_server(self) -> Result<()> {
        tracing::debug!("run webhook server");

        // TODO (@Techassi): Make opt-in configurable from the outside
        // Create an OpenTelemetry tracing layer
        tracing::trace!("create tracing service (layer)");
        let trace_layer = AxumTraceLayer::new().with_opt_in();

        // Use a service builder to provide multiple layers at once. Recommended
        // by the Axum project.
        //
        // See https://docs.rs/axum/latest/axum/middleware/index.html#applying-multiple-middleware
        // TODO (@NickLarsenNZ): rename this server_builder and keep it specific to tracing, since it's placement in the chain is important
        let service_builder = ServiceBuilder::new().layer(trace_layer);

        // Create the root router and merge the provided router into it.
        tracing::debug!("create core router and merge provided router");
        let router = self
            .router
            .layer(service_builder)
            // The health route is below the AxumTraceLayer so as not to be instrumented
            .route("/health", get(|| async { "ok" }));

        // Create server for TLS termination
        tracing::debug!("create TLS server");
        let tls_server = TlsServer::new(self.options.socket_addr, router)
            .await
            .context(CreateTlsServerSnafu)?;

        tracing::info!("running TLS server");
        tls_server.run().await.context(RunTlsServerSnafu)
    }
}
