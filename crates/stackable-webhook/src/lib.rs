use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use ::x509_cert::Certificate;
use axum::{Router, routing::get};
use futures_util::{FutureExt as _, TryFutureExt, select};
use k8s_openapi::ByteString;
use servers::{WebhookServerImplementation, WebhookServerImplementationError};
use snafu::{ResultExt, Snafu};
use stackable_telemetry::AxumTraceLayer;
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    try_join,
};
use tower::ServiceBuilder;
use x509_cert::der::{EncodePem, pem::LineEnding};

use crate::tls::TlsServer;

pub mod servers;
pub mod tls;

/// A result type alias with the [`WebhookError`] type as the default error type.
pub type Result<T, E = WebhookError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum WebhookError {
    #[snafu(display("failed to create TLS server"))]
    CreateTlsServer { source: tls::TlsServerError },

    #[snafu(display("failed to run TLS server"))]
    RunTlsServer { source: tls::TlsServerError },

    #[snafu(display("failed to update certificate"))]
    UpdateCertificate {
        source: WebhookServerImplementationError,
    },

    #[snafu(display("failed to encode CA certificate as PEM format"))]
    EncodeCertificateAuthorityAsPem { source: x509_cert::der::Error },
}

pub struct WebhookServer {
    options: WebhookOptions,
    webhooks: Vec<Box<dyn WebhookServerImplementation>>,
    tls_server: TlsServer,
    cert_rx: mpsc::Receiver<Certificate>,
}

#[derive(Clone, Debug)]
pub struct WebhookOptions {
    /// The default HTTPS socket address the [`TcpListener`][tokio::net::TcpListener]
    /// binds to.
    pub socket_addr: SocketAddr,

    /// The namespace the operator/webhook is running in.
    pub operator_namespace: String,

    /// The name of the Kubernetes service which points to the operator/webhook.
    pub operator_service_name: String,
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

    pub async fn new(
        options: WebhookOptions,
        webhooks: Vec<Box<dyn WebhookServerImplementation>>,
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
        // TODO (@NickLarsenNZ): rename this server_builder and keep it specific to tracing, since it's placement in the chain is important
        let service_builder = ServiceBuilder::new().layer(trace_layer);

        // Create the root router and merge the provided router into it.
        tracing::debug!("create core router and merge provided router");
        let mut router = Router::new()
            .layer(service_builder)
            // The health route is below the AxumTraceLayer so as not to be instrumented
            .route("/health", get(|| async { "ok" }));

        for webhook in webhooks.iter() {
            router = webhook.register_routes(router);
        }

        tracing::debug!("create TLS server");
        let (tls_server, cert_rx) = TlsServer::new(router, options.clone())
            .await
            .context(CreateTlsServerSnafu)?;

        Ok(Self {
            options,
            webhooks,
            tls_server,
            cert_rx,
        })
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
        tokio::pin!(future_server);
        tokio::pin!(future_signal);

        tokio::select! {
            res = &mut future_server => {
                // If the server future errors, propagate the error
                res?;
            }
            _ = &mut future_signal => {
                tracing::info!("shutdown signal received, stopping server");
            }
        }

        Ok(())
    }

    async fn run_server(self) -> Result<()> {
        tracing::debug!("run webhook server");

        let Self {
            options,
            mut webhooks,
            tls_server,
            mut cert_rx,
            // initial_reconcile_tx,
        } = self;
        let tls_server = tls_server
            .run()
            .map_err(|err| WebhookError::RunTlsServer { source: err });

        let cert_update_loop = async {
            loop {
                while let Some(cert) = cert_rx.recv().await {
                    // The caBundle needs to be provided as a base64-encoded PEM envelope.
                    let ca_bundle = cert
                        .to_pem(LineEnding::LF)
                        .context(EncodeCertificateAuthorityAsPemSnafu)?;
                    let ca_bundle = ByteString(ca_bundle.as_bytes().to_vec());

                    for webhook in webhooks.iter_mut() {
                        webhook
                            .handle_certificate_rotation(&cert, &ca_bundle, &options)
                            .await
                            .context(UpdateCertificateSnafu)?;
                    }
                }
            }

            // We need to hint the return type to the compiler
            #[allow(unreachable_code)]
            Ok(())
        };

        try_join!(cert_update_loop, tls_server).map(|_| ())
    }
}
