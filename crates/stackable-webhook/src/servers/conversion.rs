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
use tokio::sync::mpsc;
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
    current_cert: Certificate,

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
        let webhook_domain_name = format!(
            "{service_name}.{operator_namespace}.svc",
            service_name = operator_environment.operator_service_name,
            operator_namespace = operator_environment.operator_namespace,
        );

        let (cert_tx, mut cert_rx) = mpsc::channel(1);
        let server = WebhookServer::new(router, options, vec![webhook_domain_name], cert_tx)
            .await
            .context(CreateWebhookServerSnafu)?;
        let current_cert = cert_rx
            .recv()
            .await
            .context(ReceiverCertificateFromChannelSnafu)?;

        Ok(Self {
            server,
            current_cert,
            client,
            field_manager: field_manager.into(),
            crds,
            operator_environment,
        })
    }

    /// Starts the conversion webhook server
    ///
    /// Use [`Self::reconcile_crds`] first to avoid "too old resource version" error
    pub async fn run(self) -> Result<(), ConversionWebhookError> {
        tracing::info!("starting conversion webhook server");

        self.server.run().await.context(RunWebhookServerSnafu)?;

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn reconcile_crds(&self) -> Result<(), ConversionWebhookError> {
        tracing::info!(kinds = ?self.crds.keys(), "Reconciling CRDs");
        let ca_bundle = self
            .current_cert
            .to_pem(LineEnding::LF)
            .context(ConvertCaToPemSnafu)?;

        let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        for (kind, crd) in &self.crds {
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
                            name: self.operator_environment.operator_service_name.clone(),
                            namespace: self.operator_environment.operator_namespace.clone(),
                            path: Some(format!("/convert/{kind}")),
                            port: Some(
                                DEFAULT_HTTPS_PORT
                                    .try_into()
                                    .expect("DEFAULT_HTTPS_PORT must be convertible into i32"),
                            ),
                        }),
                        ca_bundle: Some(ByteString(ca_bundle.as_bytes().to_vec())),
                        url: None,
                    }),
                }),
            });

            // TODO: Move this into function and do a more clever update mechanism
            let crd_name = crd.name_any();
            let patch = Patch::Apply(&crd);
            let patch_params = PatchParams::apply(&self.field_manager);
            crd_api
                .patch(&crd_name, &patch_params, &patch)
                .await
                .with_context(|_| UpdateCRDSnafu {
                    crd_name: crd_name.to_string(),
                })?;
            tracing::info!(crd_name, "Reconciled CRDs");
        }
        Ok(())
    }
}
