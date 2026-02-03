//! Utility types and functions to easily create ready-to-use webhook servers which can handle
//! different tasks. All webhook servers use HTTPS by default.
//!
//! Currently the following webhooks are supported:
//!
//! * [webhooks::ConversionWebhook]
//! * [webhooks::MutatingWebhook]
//! * In the future validating webhooks wil be added
//!
//! This library is fully compatible with the  [`tracing`] crate and emits debug level tracing data.
//!
//! For usage please look at the [`WebhookServer`] docs as well as the specific [`Webhook`] you are
//! using.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use ::x509_cert::Certificate;
use axum::{Router, routing::get};
use futures_util::TryFutureExt;
use k8s_openapi::ByteString;
use snafu::{ResultExt, Snafu};
use stackable_telemetry::AxumTraceLayer;
use tokio::{sync::mpsc, try_join};
use tower::ServiceBuilder;
use webhooks::{Webhook, WebhookError};
use x509_cert::der::{EncodePem, pem::LineEnding};

use crate::tls::TlsServer;

pub mod tls;
pub mod webhooks;

/// A result type alias with the [`WebhookError`] type as the default error type.
pub type Result<T, E = WebhookServerError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum WebhookServerError {
    #[snafu(display("failed to create TLS server"))]
    CreateTlsServer { source: tls::TlsServerError },

    #[snafu(display("failed to run TLS server"))]
    RunTlsServer { source: tls::TlsServerError },

    #[snafu(display("failed to update certificate"))]
    UpdateCertificate { source: WebhookError },

    #[snafu(display("failed to encode CA certificate as PEM format"))]
    EncodeCertificateAuthorityAsPem { source: x509_cert::der::Error },
}

/// An HTTPS server that serves one or more webhooks.
///
/// It also handles TLS certificate rotation.
///
/// ### Example usage
///
/// ```
/// use stackable_webhook::{WebhookServer, WebhookServerOptions, webhooks::Webhook};
/// use tokio::time::{Duration, sleep};
///
/// # async fn docs() {
/// let mut webhooks: Vec<Box<dyn Webhook>> = vec![];
///
/// let webhook_options = WebhookServerOptions {
///     socket_addr: WebhookServer::DEFAULT_SOCKET_ADDRESS,
///     webhook_namespace: "my-namespace".to_owned(),
///     webhook_service_name: "my-operator".to_owned(),
/// };
/// let webhook_server = WebhookServer::new(webhooks, webhook_options).await.unwrap();
/// let shutdown_signal = sleep(Duration::from_millis(100));
///
/// webhook_server.run(shutdown_signal).await.unwrap();
/// # }
/// ```
pub struct WebhookServer {
    options: WebhookServerOptions,
    webhooks: Vec<Box<dyn Webhook>>,
    tls_server: TlsServer,
    cert_rx: mpsc::Receiver<Certificate>,
}

/// Configuration of a [`WebhookServer`], which is passed to [`WebhookServer::new`]
#[derive(Clone, Debug)]
pub struct WebhookServerOptions {
    /// The HTTPS socket address the [`TcpListener`][tokio::net::TcpListener] binds to.
    pub socket_addr: SocketAddr,

    /// The namespace the webhook is running in.
    pub webhook_namespace: String,

    /// The name of the Kubernetes service which points to the webhook.
    pub webhook_service_name: String,
}

impl WebhookServer {
    /// The default HTTPS port
    pub const DEFAULT_HTTPS_PORT: u16 = 8443;
    /// The default IP address [`Ipv4Addr::UNSPECIFIED`] (`0.0.0.0`) the webhook server binds to,
    /// which represents binding on all network addresses.
    //
    // TODO: We might want to switch to `Ipv6Addr::UNSPECIFIED)` here, as this *normally* binds to IPv4
    // and IPv6. However, it's complicated and depends on the underlying system...
    // If we do so, we should set `set_only_v6(false)` on the socket to not rely on system defaults.
    pub const DEFAULT_LISTEN_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    /// The default socket address `0.0.0.0:8443` the webhook server binds to.
    pub const DEFAULT_SOCKET_ADDRESS: SocketAddr =
        SocketAddr::new(Self::DEFAULT_LISTEN_ADDRESS, Self::DEFAULT_HTTPS_PORT);

    /// Creates a new webhook server with the given config and list of [`Webhook`]s.
    ///
    /// Please read their documentation for details.
    pub async fn new(
        webhooks: Vec<Box<dyn Webhook>>,
        options: WebhookServerOptions,
    ) -> Result<Self> {
        tracing::trace!("create new webhook server");

        // TODO (@Techassi): Make opt-in configurable from the outside
        // Create an OpenTelemetry tracing layer
        tracing::trace!("create tracing service (layer)");
        let trace_layer = AxumTraceLayer::new().with_opt_in();

        // Use a service builder to provide multiple layers at once. Recommended
        // by the Axum project.
        //
        // See https://docs.rs/axum/latest/axum/middleware/index.html#applying-multiple-middleware
        let trace_service_builder = ServiceBuilder::new().layer(trace_layer);

        // Create the root router and merge the provided router into it.
        tracing::debug!("create core router and merge provided router");
        let mut router = Router::new();
        for webhook in &webhooks {
            router = webhook.register_routes(router);
        }

        let router = router
            // Enrich spans for routes added above.
            // Routes defined below it will not be instrumented to reduce noise.
            .layer(trace_service_builder)
            // The health route is below the AxumTraceLayer so as not to be instrumented
            .route("/health", get(|| async { "ok" }));

        tracing::debug!("create TLS server");
        let (tls_server, cert_rx) = TlsServer::new(router, &options)
            .await
            .context(CreateTlsServerSnafu)?;

        Ok(Self {
            options,
            webhooks,
            tls_server,
            cert_rx,
        })
    }

    /// Runs the [`WebhookServer`] and handles underlying certificate rotations of the [`TlsServer`].
    ///
    /// It should be noted that the server is never started in cases where no [`Webhook`] is
    /// registered. Callers of this function need to ensure to choose the correct joining mechanism
    /// for their use-case to for example avoid unexpected shutdowns of the whole Kubernetes
    /// controller.
    pub async fn run<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: Future<Output = ()>,
    {
        tracing::debug!("run webhook server");

        let Self {
            options,
            mut webhooks,
            tls_server,
            mut cert_rx,
        } = self;

        // If no webhooks are registered exit immediately without spanning the TLS server and the
        // certificate rotation loop.
        if webhooks.is_empty() {
            tracing::debug!("no registered webhooks, returning without starting TLS server");
            return Ok(());
        }

        let tls_server = tls_server
            .run(shutdown_signal)
            .map_err(|err| WebhookServerError::RunTlsServer { source: err });

        let cert_update_loop = async {
            // Once the shutdown signal is triggered, the TlsServer above should be dropped as the
            // run associated function consumes self. This in turn means that when the receiver is
            // polled, it will return `Ok(Ready(None))`, which will cause this while loop to break
            // and the future to complete.
            while let Some(certificate) = cert_rx.recv().await {
                // NOTE (@Techassi): There are currently NO semantic conventions for X509 certificates
                // and as such, these are pretty much made up and potentially not ideal.
                #[rustfmt::skip]
                tracing::info!(
                    x509.not_before = certificate.tbs_certificate.validity.not_before.to_string(),
                    x509.not_after = certificate.tbs_certificate.validity.not_after.to_string(),
                    x509.serial_number = certificate.tbs_certificate.serial_number.to_string(),
                    x509.subject = certificate.tbs_certificate.subject.to_string(),
                    x509.issuer = certificate.tbs_certificate.issuer.to_string(),
                    "rotate certificate for registered webhooks"
                );

                // The caBundle needs to be provided as a base64-encoded PEM envelope.
                let ca_bundle = certificate
                    .to_pem(LineEnding::LF)
                    .context(EncodeCertificateAuthorityAsPemSnafu)?;
                let ca_bundle = ByteString(ca_bundle.as_bytes().to_vec());

                for webhook in webhooks.iter_mut() {
                    if webhook.ignore_certificate_rotation() {
                        continue;
                    }

                    webhook
                        .handle_certificate_rotation(&ca_bundle, &options)
                        .await
                        .context(UpdateCertificateSnafu)?;
                }
            }

            Ok(())
        };

        // This either returns if one of the two futures completes with Err(_) or when both complete
        // with Ok(_). Both futures complete with Ok(_) when a shutdown signal is received.
        try_join!(cert_update_loop, tls_server).map(|_| ())
    }
}
