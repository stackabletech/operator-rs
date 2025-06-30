use std::{collections::HashMap, fmt::Debug};

use axum::{Json, Router, routing::post};
use k8s_openapi::{
    ByteString,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::{
        CustomResourceConversion, CustomResourceDefinition, ServiceReference, WebhookClientConfig,
        WebhookConversion,
    },
};
// Re-export this type because users of the conversion webhook server require
// this type to write the handler function. Instead of importing this type from
// kube directly, consumers can use this type instead. This also eliminates
// keeping the kube dependency version in sync between here and the operator.
pub use kube::core::conversion::ConversionReview;
use kube::{
    Api, Client, ResourceExt,
    api::{Patch, PatchParams},
};
use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::cli::OperatorEnvironmentOpts;
use tokio::{sync::mpsc, try_join};
use tracing::instrument;
use x509_cert::{
    Certificate,
    der::{EncodePem, pem::LineEnding},
};

use crate::{
    WebhookError, WebhookHandler, WebhookServer, constants::DEFAULT_HTTPS_PORT, options::Options,
};

#[derive(Debug, Snafu)]
pub enum ConversionWebhookError {
    #[snafu(display("failed to create webhook server"))]
    CreateWebhookServer { source: WebhookError },

    #[snafu(display("failed to run webhook server"))]
    RunWebhookServer { source: WebhookError },

    #[snafu(display("failed to receive certificate from channel"))]
    ReceiverCertificateFromChannel,

    #[snafu(display("failed to convert CA certificate into PEM format"))]
    ConvertCaToPem { source: x509_cert::der::Error },

    #[snafu(display("failed to reconcile CRDs"))]
    ReconcileCRDs {
        #[snafu(source(from(ConversionWebhookError, Box::new)))]
        source: Box<ConversionWebhookError>,
    },

    #[snafu(display("failed to update CRD {crd_name:?}"))]
    UpdateCRD {
        source: stackable_operator::kube::Error,
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

/// A ready-to-use CRD conversion webhook server.
///
/// See [`ConversionWebhookServer::new()`] for usage examples.
pub struct ConversionWebhookServer {
    server: WebhookServer,
    cert_rx: mpsc::Receiver<Certificate>,
    client: Client,
    field_manager: String,
    crds: HashMap<String, CustomResourceDefinition>,
    operator_environment: OperatorEnvironmentOpts,
}

impl ConversionWebhookServer {
    /// Creates a new conversion webhook server, which expects POST requests being made to the
    /// `/convert/{crd name}` endpoint.
    ///
    /// You need to provide two things for every CRD passed in via the `crds_and_handlers` argument:
    ///
    /// 1. The CRD
    /// 2. A conversion function to convert between CRD versions. Typically you would use the
    ///   the auto-generated `try_convert` function on CRD spec definition structs for this.
    ///
    /// The [`ConversionWebhookServer`] takes care of reconciling the CRDs into the Kubernetes
    /// cluster and takes care of adding itself as conversion webhook. This includes TLS
    /// certificates and CA bundles.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use stackable_webhook::{
    ///     servers::{ConversionReview, ConversionWebhookServer},
    ///     Options
    /// };
    /// use stackable_operator::cli::OperatorEnvironmentOpts;
    /// use stackable_operator::kube::Client;
    /// use stackable_operator::crd::s3::{S3Connection, S3ConnectionVersion};
    ///
    /// # async fn test() {
    /// let crds_and_handlers = [
    ///     (
    ///         S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1).unwrap(),
    ///         S3Connection::try_convert as fn(ConversionReview) -> ConversionReview,
    ///     ),
    /// ];
    ///
    /// const OPERATOR_NAME: &str = "PRODUCT_OPERATOR";
    /// let client = Client::try_default().await.expect("failed to create Kubernetes client");
    /// // Normally you would get this from the CLI arguments in `ProductOperatorRun::operator_environment`
    /// let operator_environment = OperatorEnvironmentOpts {
    ///     operator_namespace: "stackable-operator".to_string(),
    ///     operator_service_name: "product-operator".to_string(),
    /// };
    ///
    /// // Construct the conversion webhook server
    /// let conversion_webhook = ConversionWebhookServer::new(
    ///     crds_and_handlers,
    ///     stackable_webhook::Options::default(),
    ///     client,
    ///     OPERATOR_NAME,
    ///     operator_environment,
    /// )
    /// .await
    /// .expect("failed to create ConversionWebhookServer");
    ///
    /// // Bootstrap CRDs first to avoid "too old resource version" error
    /// conversion_webhook.reconcile_crds().await.expect("failed to reconcile CRDs");
    /// # }
    /// ```
    #[instrument(
        name = "create_conversion_webhook_server",
        skip(crds_and_handlers, client)
    )]
    pub async fn new<H>(
        crds_and_handlers: impl IntoIterator<Item = (CustomResourceDefinition, H)>,
        options: Options,
        client: Client,
        field_manager: impl Into<String> + Debug,
        operator_environment: OperatorEnvironmentOpts,
    ) -> Result<Self, ConversionWebhookError>
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        tracing::debug!("create new conversion webhook server");
        let field_manager: String = field_manager.into();

        let mut router = Router::new();
        let mut crds = HashMap::new();
        for (crd, handler) in crds_and_handlers {
            let crd_name = crd.name_any();
            let handler_fn = |Json(review): Json<ConversionReview>| async {
                let review = handler.call(review);
                Json(review)
            };

            router = router.route(&format!("/convert/{crd_name}"), post(handler_fn));
            crds.insert(crd_name, crd);
        }

        // This is how Kubernetes calls us, so it decides about the naming.
        // AFAIK we can not influence this, so this is the only SAN entry needed.
        let sans = vec![format!(
            "{service_name}.{operator_namespace}.svc",
            service_name = operator_environment.operator_service_name,
            operator_namespace = operator_environment.operator_namespace,
        )];

        let (server, mut cert_rx) = WebhookServer::new(router, options, sans)
            .await
            .context(CreateWebhookServerSnafu)?;

        // We block the ConversionWebhookServer creation until the certificates have been generated.
        // This way we
        // 1. Are able to apply the CRDs before we start the actual controllers relying on them
        // 2. Avoid updating them shortly after as cert have been generated. Doing so would cause
        // unnecessary "too old resource version" errors in the controllers as the CRD was updated.
        let current_cert = cert_rx
            .recv()
            .await
            .context(ReceiverCertificateFromChannelSnafu)?;
        Self::reconcile_crds(
            &client,
            &field_manager,
            &crds,
            &operator_environment,
            &current_cert,
        )
        .await
        .context(ReconcileCRDsSnafu)?;

        Ok(Self {
            server,
            cert_rx,
            client,
            field_manager,
            crds,
            operator_environment,
        })
    }

    pub async fn run(self) -> Result<(), ConversionWebhookError> {
        tracing::info!("starting conversion webhook server");

        let Self {
            server,
            cert_rx,
            client,
            field_manager,
            crds,
            operator_environment,
        } = self;

        try_join!(
            Self::run_webhook_server(server),
            Self::run_cert_update_loop(
                cert_rx,
                &client,
                &field_manager,
                &crds,
                &operator_environment
            ),
        )?;

        Ok(())
    }

    async fn run_webhook_server(server: WebhookServer) -> Result<(), ConversionWebhookError> {
        server.run().await.context(RunWebhookServerSnafu)
    }

    async fn run_cert_update_loop(
        mut cert_rx: mpsc::Receiver<Certificate>,
        client: &Client,
        field_manager: &str,
        crds: &HashMap<String, CustomResourceDefinition>,
        operator_environment: &OperatorEnvironmentOpts,
    ) -> Result<(), ConversionWebhookError> {
        while let Some(current_cert) = cert_rx.recv().await {
            Self::reconcile_crds(
                client,
                field_manager,
                crds,
                operator_environment,
                &current_cert,
            )
            .await
            .context(ReconcileCRDsSnafu)?;
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn reconcile_crds(
        client: &Client,
        field_manager: &str,
        crds: &HashMap<String, CustomResourceDefinition>,
        operator_environment: &OperatorEnvironmentOpts,
        current_cert: &Certificate,
    ) -> Result<(), ConversionWebhookError> {
        tracing::info!(kinds = ?crds.keys(), "Reconciling CRDs");
        let ca_bundle = current_cert
            .to_pem(LineEnding::LF)
            .context(ConvertCaToPemSnafu)?;

        let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
        for (kind, crd) in crds {
            let mut crd = crd.clone();

            crd.spec.conversion = Some(CustomResourceConversion {
                strategy: "Webhook".to_string(),
                webhook: Some(WebhookConversion {
                    // conversionReviewVersions indicates what ConversionReview versions are understood/preferred by the webhook.
                    // The first version in the list understood by the API server is sent to the webhook.
                    // The webhook must respond with a ConversionReview object in the same version it received.
                    conversion_review_versions: vec!["v1".to_string()],
                    client_config: Some(WebhookClientConfig {
                        service: Some(ServiceReference {
                            name: operator_environment.operator_service_name.clone(),
                            namespace: operator_environment.operator_namespace.clone(),
                            path: Some(format!("/convert/{kind}")),
                            port: Some(DEFAULT_HTTPS_PORT.into()),
                        }),
                        ca_bundle: Some(ByteString(ca_bundle.as_bytes().to_vec())),
                        url: None,
                    }),
                }),
            });

            // TODO: Move this into function and do a more clever update mechanism
            let crd_name = crd.name_any();
            let patch = Patch::Apply(&crd);
            let patch_params = PatchParams::apply(field_manager);
            crd_api
                .patch(&crd_name, &patch_params, &patch)
                .await
                .with_context(|_| UpdateCRDSnafu {
                    crd_name: crd_name.to_string(),
                })?;
            tracing::info!(crd.name = crd_name, "Reconciled CRD");
        }
        Ok(())
    }
}
