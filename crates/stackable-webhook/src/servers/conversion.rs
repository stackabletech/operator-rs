use std::{fmt::Debug, net::SocketAddr};

use axum::{Json, Router, routing::post};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
// Re-export this type because users of the conversion webhook server require
// this type to write the handler function. Instead of importing this type from
// kube directly, consumers can use this type instead. This also eliminates
// keeping the kube dependency version in sync between here and the operator.
pub use kube::core::conversion::ConversionReview;
use kube::{Client, ResourceExt};
use snafu::{ResultExt, Snafu};
use tokio::sync::{mpsc, oneshot};
use tracing::instrument;
use x509_cert::Certificate;

use crate::{
    WebhookError, WebhookHandler, WebhookServer,
    maintainer::{CustomResourceDefinitionMaintainer, CustomResourceDefinitionMaintainerOptions},
    options::WebhookOptions,
};

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
pub struct ConversionWebhookOptions<'a> {
    /// The bind address to bind the HTTPS server to.
    pub socket_addr: SocketAddr,

    /// The namespace the operator/webhook is running in.
    pub namespace: &'a str,

    /// The name of the Kubernetes service which points to the operator/webhook.
    pub service_name: &'a str,
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
    /// The TLS certificate is automatically generated and rotated.
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
    /// ```no_run
    /// # use tokio_rustls::rustls::crypto::{CryptoProvider, ring::default_provider};
    /// use stackable_webhook::servers::{ConversionWebhookServer, ConversionWebhookOptions};
    /// use stackable_operator::crd::s3::{S3Connection, S3ConnectionVersion};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # CryptoProvider::install_default(default_provider()).unwrap();
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
    ///     namespace: "stackable-operators",
    ///     service_name: "product-operator",
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
        options: ConversionWebhookOptions<'_>,
    ) -> Result<(Self, mpsc::Receiver<Certificate>), ConversionWebhookError>
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        tracing::debug!("create new conversion webhook server");

        let mut router = Router::new();

        for (crd, handler) in crds_and_handlers {
            let crd_name = crd.name_any();
            let handler_fn = |Json(review): Json<ConversionReview>| async {
                let review = handler.call(review);
                Json(review)
            };

            // TODO (@Techassi): Make this part of the trait mentioned above
            let route = format!("/convert/{crd_name}");
            router = router.route(&route, post(handler_fn));
        }

        let ConversionWebhookOptions {
            socket_addr,
            namespace: operator_namespace,
            service_name: operator_service_name,
        } = &options;

        // This is how Kubernetes calls us, so it decides about the naming.
        // AFAIK we can not influence this, so this is the only SAN entry needed.
        // TODO (@Techassi): The cluster domain should be included here, so that (non Kubernetes)
        // HTTP clients can use the FQDN of the service for testing or user use-cases.
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

    /// Creates and returns a tuple consisting of a [`ConversionWebhookServer`], a [`CustomResourceDefinitionMaintainer`],
    /// and a [`oneshot::Receiver`].
    ///
    /// ## Parameters
    ///
    /// - `crds_and_handlers`: An iterator over a 2-tuple (pair) mapping a [`CustomResourceDefinition`]
    ///   to a handler function. In most cases, the generated `CustomResource::try_merge` function
    ///   should be used. It provides the expected `fn(ConversionReview) -> ConversionReview`
    ///   signature.
    /// - `operator_name`: The name of the operator. This is used to construct the webhook service
    ///   name.
    /// - `operator_namespace`: The namespace the operator runs in. This is used to construct the
    ///   webhook service name.
    /// - `disable_maintainer`: A boolean value to indicate if the [`CustomResourceDefinitionMaintainer`]
    ///   should be disabled.
    /// - `client`: A [`kube::Client`] used to maintain the custom resource definitions.
    ///
    /// See the referenced items for more details on usage.
    ///
    /// ## Return Values
    ///
    /// - The [`ConversionWebhookServer`] itself. This is used to run the server. See
    ///   [`ConversionWebhookServer::run`] for more details.
    /// - The [`CustomResourceDefinitionMaintainer`] which is used to run the maintainer. See
    ///   [`CustomResourceDefinitionMaintainer::run`] for more details.
    /// - A [`oneshot::Receiver`] which is triggered after the initial reconciliation of the CRDs
    ///   succeeded. This signal can be used to deploy any custom resources defined by these CRDs.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # use futures_util::TryFutureExt;
    /// # use tokio_rustls::rustls::crypto::{CryptoProvider, ring::default_provider};
    /// use stackable_webhook::servers::{ConversionWebhookServer, ConversionWebhookOptions};
    /// use stackable_operator::{kube::Client, crd::s3::{S3Connection, S3ConnectionVersion}};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # CryptoProvider::install_default(default_provider()).unwrap();
    /// let client = Client::try_default().await.unwrap();
    ///
    /// let crds_and_handlers = vec![
    ///     (
    ///         S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1)
    ///             .expect("the S3Connection CRD must be merged"),
    ///         S3Connection::try_convert,
    ///     )
    /// ];
    ///
    /// let (conversion_webhook_server, crd_maintainer, _initial_reconcile_rx) =
    ///     ConversionWebhookServer::with_maintainer(
    ///         crds_and_handlers,
    ///         "my-operator",
    ///         "my-namespace",
    ///         false,
    ///         client,
    ///     )
    ///     .await
    ///     .unwrap();
    ///
    /// let conversion_webhook_server = conversion_webhook_server
    ///     .run()
    ///     .map_err(|err| err.to_string());
    ///
    /// let crd_maintainer = crd_maintainer
    ///     .run()
    ///     .map_err(|err| err.to_string());
    ///
    /// // Run both the conversion webhook server and crd_maintainer concurrently, eg. with
    /// // futures::try_join!.
    /// futures_util::try_join!(conversion_webhook_server, crd_maintainer).unwrap();
    /// # }
    /// ```
    pub async fn with_maintainer<'a, H>(
        // TODO (@Techassi): Use a trait type here which can be used to build all part of the
        // conversion webhook server and a CRD maintainer.
        crds_and_handlers: impl IntoIterator<Item = (CustomResourceDefinition, H)> + Clone,
        operator_name: &'a str,
        operator_namespace: &'a str,
        field_manager: &'a str,
        disable_maintainer: bool,
        client: Client,
    ) -> Result<
        (
            Self,
            CustomResourceDefinitionMaintainer<'a>,
            oneshot::Receiver<()>,
        ),
        ConversionWebhookError,
    >
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        let socket_addr = ConversionWebhookServer::DEFAULT_SOCKET_ADDRESS;

        // TODO (@Techassi): These should be moved into a builder
        let webhook_options = ConversionWebhookOptions {
            namespace: operator_namespace,
            service_name: operator_name,
            socket_addr,
        };

        let (conversion_webhook_server, certificate_rx) =
            Self::new(crds_and_handlers.clone(), webhook_options).await?;

        let definitions = crds_and_handlers.into_iter().map(|(crd, _)| crd);

        // TODO (@Techassi): These should be moved into a builder
        let maintainer_options = CustomResourceDefinitionMaintainerOptions {
            webhook_https_port: socket_addr.port(),
            disabled: disable_maintainer,
            operator_namespace,
            operator_name,
            field_manager,
        };

        let (maintainer, initial_reconcile_rx) = CustomResourceDefinitionMaintainer::new(
            client,
            certificate_rx,
            definitions,
            maintainer_options,
        );

        Ok((conversion_webhook_server, maintainer, initial_reconcile_rx))
    }

    /// Runs the [`ConversionWebhookServer`] asynchronously.
    pub async fn run(self) -> Result<(), ConversionWebhookError> {
        tracing::info!("run conversion webhook server");
        self.0.run().await.context(RunWebhookServerSnafu)
    }
}
