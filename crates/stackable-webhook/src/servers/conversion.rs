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

    /// Creates and returns a new [`ConversionWebhookServer`], which expects POST requests being
    /// made to the `/convert/{CRD_NAME}` endpoint.
    ///
    /// ## Parameters
    ///
    /// This function expects the following parameters:
    ///
    /// - `crds_and_handlers`: An iterator over a 2-tuple (pair) mapping a [`CustomResourceDefinition`]
    ///   to a handler function. In most cases, the generated `CustomResource::try_merge` function
    ///   should be used. It provides the expected `fn(ConversionReview) -> ConversionReview`
    ///   signature.
    /// - `options`: Provides [`ConversionWebhookOptions`] to customize various parts of the
    ///   webhook server, eg. the socket address used to listen on.
    ///
    /// ## Return Values
    ///
    /// This function returns a [`Result`] which contains a 2-tuple (pair) of values for the [`Ok`]
    /// variant:
    ///
    /// - The [`ConversionWebhookServer`] itself. This is used to run the server. See
    ///   [`ConversionWebhookServer::run`] for more details.
    /// - The [`mpsc::Receiver`] which will be used to send out messages containing the newly
    ///   generated TLS certificate. This channel is used by the CRD maintainer to trigger a
    ///   reconcile of the CRDs it maintains.
    ///
    /// ## Example
    ///
    /// ```
    /// use stackable_webhook::{ConversionWebhookServer, ConversionWebhookOptions};
    /// use stackable_operator::crd::s3::{S3Connection, S3ConnectionVersion};
    ///
    /// # #[tokio::test]
    /// # async fn main() {
    /// let crds_and_handlers = vec![
    ///     (
    ///         S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1)
    ///             .expect("the S3Connection CRD must be merged"),
    ///         S3Connection::try_convert,
    ///     )
    /// ];
    ///
    /// let options = ConversionWebhookOptions {
    ///     socket_addr: ConversionWebhookServer::DEFAULT_SOCKET_ADDRESS,
    ///     namespace: "stackable-operators".to_owned(),
    ///     service_name: "product-operator".to_owned(),
    /// };
    ///
    /// let (conversion_webhook_server, _certificate_rx) =
    ///         ConversionWebhookServer::new(crds_and_handlers, options)
    ///             .await
    ///             .unwrap();
    ///
    /// conversion_webhook_server.run().await.unwrap();
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
