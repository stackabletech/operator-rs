//! This module contains structs and functions to easily create a TLS termination
//! server, which can be used in combination with an Axum [`Router`].
use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{ConnectInfo, Request},
    middleware::AddExtension,
};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use opentelemetry::trace::{FutureExt, SpanKind};
use snafu::{ResultExt, Snafu};
use stackable_operator::time::Duration;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        crypto::ring::default_provider,
        version::{TLS12, TLS13},
    },
};
use tower::{Service, ServiceExt};
use tracing::{Instrument, Span, field::Empty, instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use x509_cert::Certificate;

mod cert_resolver;

pub use cert_resolver::{CertificateResolver, CertificateResolverError};

pub const WEBHOOK_CA_LIFETIME: Duration = Duration::from_minutes_unchecked(3);
pub const WEBHOOK_CERTIFICATE_LIFETIME: Duration = Duration::from_minutes_unchecked(2);
pub const WEBHOOK_CERTIFICATE_ROTATION_INTERVAL: Duration = Duration::from_minutes_unchecked(1);

pub type Result<T, E = TlsServerError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum TlsServerError {
    #[snafu(display("failed to create certificate resolver"))]
    CreateCertificateResolver { source: CertificateResolverError },

    #[snafu(display("failed to create TCP listener by binding to socket address {socket_addr:?}"))]
    BindTcpListener {
        source: std::io::Error,
        socket_addr: SocketAddr,
    },

    #[snafu(display("failed to rotate certificate"))]
    RotateCertificate { source: CertificateResolverError },

    #[snafu(display("failed to set safe TLS protocol versions"))]
    SetSafeTlsProtocolVersions { source: tokio_rustls::rustls::Error },
}

/// A server which terminates TLS connections and allows clients to communicate
/// via HTTPS with the underlying HTTP router.
///
/// It also rotates the generated certificates as needed.
pub struct TlsServer {
    config: ServerConfig,
    cert_resolver: Arc<CertificateResolver>,

    socket_addr: SocketAddr,
    router: Router,
}

impl TlsServer {
    /// Create a new [`TlsServer`].
    ///
    /// This create a [`CertificateResolver`] with the provided `subject_alterative_dns_names`,
    /// which takes care of the certificate rotation. Afterwards it create the [`ServerConfig`],
    /// which let's the [`CertificateResolver`] provide the needed certificates.
    #[instrument(name = "create_tls_server", skip(router))]
    pub async fn new(
        socket_addr: SocketAddr,
        router: Router,
        subject_alterative_dns_names: Vec<String>,
    ) -> Result<(Self, mpsc::Receiver<Certificate>)> {
        let (cert_tx, cert_rx) = mpsc::channel(1);

        let cert_resolver = CertificateResolver::new(subject_alterative_dns_names, cert_tx)
            .await
            .context(CreateCertificateResolverSnafu)?;
        let cert_resolver = Arc::new(cert_resolver);

        let tls_provider = default_provider();
        let mut config = ServerConfig::builder_with_provider(tls_provider.into())
            .with_protocol_versions(&[&TLS12, &TLS13])
            .context(SetSafeTlsProtocolVersionsSnafu)?
            .with_no_client_auth()
            .with_cert_resolver(cert_resolver.clone());
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let tls_server = Self {
            config,
            cert_resolver,
            socket_addr,
            router,
        };

        Ok((tls_server, cert_rx))
    }

    /// Runs the TLS server by listening for incoming TCP connections on the
    /// bound socket address. It only accepts TLS connections. Internally each
    /// TLS stream get handled by a Hyper service, which in turn is an Axum
    /// router.
    ///
    /// It also starts a background task to rotate the certificate as needed.
    pub async fn run(self) -> Result<()> {
        let start = tokio::time::Instant::now() + *WEBHOOK_CERTIFICATE_ROTATION_INTERVAL;
        let mut interval = tokio::time::interval_at(start, *WEBHOOK_CERTIFICATE_ROTATION_INTERVAL);

        let tls_acceptor = TlsAcceptor::from(Arc::new(self.config));
        let tcp_listener =
            TcpListener::bind(self.socket_addr)
                .await
                .context(BindTcpListenerSnafu {
                    socket_addr: self.socket_addr,
                })?;

        // To be able to extract the connect info from incoming requests, it is
        // required to turn the router into a Tower service which is capable of
        // doing that. Calling `into_make_service_with_connect_info` returns a
        // new struct `IntoMakeServiceWithConnectInfo` which implements the
        // Tower Service trait. This service is called after the TCP connection
        // has been accepted.
        //
        // Inspired by:
        // - https://github.com/tokio-rs/axum/discussions/2397
        // - https://github.com/tokio-rs/axum/blob/b02ce307371a973039018a13fa012af14775948c/examples/serve-with-hyper/src/main.rs#L98

        let mut router = self
            .router
            .into_make_service_with_connect_info::<SocketAddr>();

        loop {
            let tls_acceptor = tls_acceptor.clone();

            // Wait for either a new TCP connection or the certificate rotation interval tick
            tokio::select! {
                // We opt for a biased execution of arms to make sure we always check if the
                // certificate needs rotation based on the interval. This ensures, we always use
                // a valid certificate for the TLS connection.
                biased;

                // This is cancellation-safe. If this branch is cancelled, the tick is NOT consumed.
                // As such, we will not miss rotating the certificate.
                _ = interval.tick() => {
                    self.cert_resolver
                        .rotate_certificate()
                        .await
                        .context(RotateCertificateSnafu)?
                }

                // This is cancellation-safe. If cancelled, no new connections are accepted.
                tcp_connection = tcp_listener.accept() => {
                    let (tcp_stream, remote_addr) = match tcp_connection {
                        Ok((stream, addr)) => (stream, addr),
                        Err(err) => {
                            tracing::trace!(%err, "failed to accept incoming TCP connection");
                            continue;
                        }
                    };

                    // Here, the connect info is extracted by calling Tower's Service
                    // trait function on `IntoMakeServiceWithConnectInfo`
                    let tower_service: Result<_, Infallible> = router.call(remote_addr).await;
                    let tower_service = tower_service.expect("Infallible error can never happen");

                    let span = tracing::debug_span!("accept tcp connection");
                    tokio::spawn(async move {
                        Self::handle_request(tcp_stream, remote_addr, tls_acceptor, tower_service, self.socket_addr)
                    }.instrument(span));
                }
            };
        }
    }

    async fn handle_request(
        tcp_stream: TcpStream,
        remote_addr: SocketAddr,
        tls_acceptor: TlsAcceptor,
        tower_service: AddExtension<Router, ConnectInfo<SocketAddr>>,
        socket_addr: SocketAddr,
    ) {
        let span = tracing::trace_span!(
            "accept tls connection",
            "otel.kind" = ?SpanKind::Server,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
            "client.address" = remote_addr.ip().to_string(),
            "client.port" = remote_addr.port() as i64,
            "server.address" = Empty,
            "server.port" = Empty,
            "network.peer.address" = remote_addr.ip().to_string(),
            "network.peer.port" = remote_addr.port() as i64,
            "network.local.address" = Empty,
            "network.local.port" = Empty,
            "network.transport" = "tcp",
            "network.type" = socket_addr.semantic_convention_network_type(),
        );

        if let Ok(local_addr) = tcp_stream.local_addr() {
            let addr = &local_addr.ip().to_string();
            let port = local_addr.port();
            span.record("server.address", addr)
                .record("server.port", port as i64)
                .record("network.local.address", addr)
                .record("network.local.port", port as i64);
        }

        // Wait for tls handshake to happen
        let tls_stream = match tls_acceptor
            .accept(tcp_stream)
            .instrument(span.clone())
            .await
        {
            Ok(tls_stream) => tls_stream,
            Err(err) => {
                span.record("otel.status_code", "Error")
                    .record("otel.status_message", err.to_string());
                tracing::trace!(%remote_addr, "error during tls handshake connection");
                return;
            }
        };

        // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
        // `TokioIo` converts between them.
        let tls_stream = TokioIo::new(tls_stream);

        // Hyper also has its own `Service` trait and doesn't use tower. We can use
        // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
        // `tower::Service::call`.
        let hyper_service = service_fn(move |request: Request<Incoming>| {
            // This carries the current context with the trace id so that the TraceLayer can use that as a parent
            let otel_context = Span::current().context();
            // We need to clone here, because oneshot consumes self
            tower_service
                .clone()
                .oneshot(request)
                .with_context(otel_context)
        });

        let span = tracing::debug_span!("serve connection");
        hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
            .serve_connection_with_upgrades(tls_stream, hyper_service)
            .instrument(span.clone())
            .await
            .unwrap_or_else(|err| {
                span.record("otel.status_code", "Error")
                    .record("otel.status_message", err.to_string());
                tracing::warn!(%err, %remote_addr, "failed to serve connection");
            })
    }
}

pub trait SocketAddrExt {
    fn semantic_convention_network_type(&self) -> &'static str;
}

impl SocketAddrExt for SocketAddr {
    fn semantic_convention_network_type(&self) -> &'static str {
        match self {
            SocketAddr::V4(_) => "ipv4",
            SocketAddr::V6(_) => "ipv6",
        }
    }
}

// TODO (@NickLarsenNZ): impl record_error(err: impl Error) for Span as a shortcut to set otel.status_* fields
// TODO (@NickLarsenNZ): wrap tracing::span macros to automatically add otel fields
