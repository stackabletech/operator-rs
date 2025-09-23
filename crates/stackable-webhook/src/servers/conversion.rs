use std::{fmt::Debug, net::SocketAddr};

use axum::{Json, Router, routing::post};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::ResourceExt;
// Re-export this type because users of the conversion webhook server require
// this type to write the handler function. Instead of importing this type from
// kube directly, consumers can use this type instead. This also eliminates
// keeping the kube dependency version in sync between here and the operator.
pub use kube::core::conversion::ConversionReview;
use snafu::{ResultExt, Snafu};
use tokio::sync::mpsc;
use tracing::instrument;
use x509_cert::Certificate;

use crate::{WebhookError, WebhookHandler, WebhookServer, options::WebhookOptions};

#[derive(Debug, Snafu)]
pub enum ConversionWebhookError {
    #[snafu(display("failed to create webhook server"))]
    CreateWebhookServer { source: WebhookError },

    #[snafu(display("failed to run webhook server"))]
    RunWebhookServer { source: WebhookError },

    #[snafu(display("failed to receive certificate from channel"))]
    ReceiveCertificateFromChannel,

    #[snafu(display("failed to convert CA certificate into PEM format"))]
    ConvertCaToPem { source: x509_cert::der::Error },

    #[snafu(display("failed to reconcile CRDs"))]
    ReconcileCrds {
        #[snafu(source(from(ConversionWebhookError, Box::new)))]
        source: Box<ConversionWebhookError>,
    },

    #[snafu(display("failed to update CRD {crd_name:?}"))]
    UpdateCrd {
        source: kube::Error,
        crd_name: String,
    },
}

impl<F> WebhookHandler<ConversionReview, ConversionReview> for F
where
    F: FnOnce(ConversionReview) -> ConversionReview,
{
    fn call(self, req: ConversionReview) -> ConversionReview {
        self(req)
    }
}

// TODO: Add a builder, maybe with `bon`.
#[derive(Debug)]
pub struct ConversionWebhookOptions {
    /// The bind address to bind the HTTPS server to.
    pub socket_addr: SocketAddr,

    /// The namespace the operator/webhook is running in.
    pub namespace: String,

    /// The name of the Kubernetes service which points to the operator/webhook.
    pub service_name: String,
}

/// A ready-to-use CRD conversion webhook server.
///
/// See [`ConversionWebhookServer::new()`] for usage examples.
pub struct ConversionWebhookServer(WebhookServer);

impl ConversionWebhookServer {
    /// The default socket address the conversion webhook server binds to, see
    /// [`WebhookServer::DEFAULT_SOCKET_ADDRESS`].
    pub const DEFAULT_SOCKET_ADDRESS: SocketAddr = WebhookServer::DEFAULT_SOCKET_ADDRESS;

    /// Creates a new conversion webhook server, which expects POST requests being made to the
    /// `/convert/{CRD_NAME}` endpoint.
    ///
    /// You need to provide a few things for every CRD passed in via the `crds_and_handlers` argument:
    ///
    /// 1. The CRD
    /// 2. A conversion function to convert between CRD versions. Typically you would use the
    ///    the auto-generated `try_convert` function on CRD spec definition structs for this.
    /// 3. A [`kube::Client`] used to create/update the CRDs.
    ///
    /// The [`ConversionWebhookServer`] takes care of reconciling the CRDs into the Kubernetes
    /// cluster and takes care of adding itself as conversion webhook. This includes TLS
    /// certificates and CA bundles.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use clap::Parser;
    /// use stackable_webhook::{
    ///     servers::{ConversionWebhookServer, ConversionWebhookOptions},
    ///     constants::CONVERSION_WEBHOOK_HTTPS_PORT,
    ///     WebhookOptions
    /// };
    /// use stackable_operator::{
    ///     kube::Client,
    ///     crd::s3::{S3Connection, S3ConnectionVersion},
    ///     cli::{RunArguments, MaintenanceOptions},
    /// };
    ///
    /// # async fn test() {
    /// // Things that should already be in you operator:
    /// const OPERATOR_NAME: &str = "product-operator";
    /// let client = Client::try_default().await.expect("failed to create Kubernetes client");
    /// let RunArguments {
    ///     operator_environment,
    ///     maintenance: MaintenanceOptions {
    ///         disable_crd_maintenance,
    ///         ..
    ///     },
    ///     ..
    /// } = RunArguments::parse();
    ///
    ///  let crds_and_handlers = [
    ///     (
    ///         S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1)
    ///             .expect("failed to merge S3Connection CRD"),
    ///         S3Connection::try_convert as fn(_) -> _,
    ///     ),
    /// ];
    ///
    /// let options = ConversionWebhookOptions {
    ///     socket_addr: format!("0.0.0.0:{CONVERSION_WEBHOOK_HTTPS_PORT}")
    ///         .parse()
    ///         .expect("static address is always valid"),
    ///     namespace: operator_environment.operator_namespace,
    ///     service_name: operator_environment.operator_service_name,
    ///     maintain_crds: !disable_crd_maintenance,
    ///     field_manager: OPERATOR_NAME.to_owned(),
    /// };
    ///
    /// // Construct the conversion webhook server
    /// let conversion_webhook = ConversionWebhookServer::new(
    ///     crds_and_handlers,
    ///     options,
    ///     client,
    /// )
    /// .await
    /// .expect("failed to create ConversionWebhookServer");
    ///
    /// conversion_webhook.run().await.expect("failed to run ConversionWebhookServer");
    /// # }
    /// ```
    #[instrument(name = "create_conversion_webhook_server", skip(crds_and_handlers))]
    pub async fn new<H>(
        crds_and_handlers: impl IntoIterator<Item = (CustomResourceDefinition, H)>,
        options: ConversionWebhookOptions,
    ) -> Result<(Self, mpsc::Receiver<Certificate>), ConversionWebhookError>
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        tracing::debug!("create new conversion webhook server");

        let mut router = Router::new();
        let mut crds = Vec::new();
        for (crd, handler) in crds_and_handlers {
            let crd_name = crd.name_any();
            let handler_fn = |Json(review): Json<ConversionReview>| async {
                let review = handler.call(review);
                Json(review)
            };

            let route = format!("/convert/{crd_name}");
            router = router.route(&route, post(handler_fn));
            crds.push(crd);
        }

        let ConversionWebhookOptions {
            socket_addr,
            namespace: operator_namespace,
            service_name: operator_service_name,
        } = &options;

        // This is how Kubernetes calls us, so it decides about the naming.
        // AFAIK we can not influence this, so this is the only SAN entry needed.
        // FIXME (@Techassi): The cluster domain should be included here to form FQDN of the service
        let subject_alterative_dns_name =
            format!("{operator_service_name}.{operator_namespace}.svc",);

        let webhook_options = WebhookOptions {
            subject_alterative_dns_names: vec![subject_alterative_dns_name],
            socket_addr: *socket_addr,
        };

        let (server, certificate_rx) = WebhookServer::new(router, webhook_options)
            .await
            .context(CreateWebhookServerSnafu)?;

        Ok((Self(server), certificate_rx))
    }

    /// Runs the [`ConversionWebhookServer`] asynchronously.
    pub async fn run(self) -> Result<(), ConversionWebhookError> {
        tracing::info!("starting conversion webhook server");
        self.0.run().await.context(RunWebhookServerSnafu)
    }
}
