use std::{fmt::Debug, net::SocketAddr};

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
use tokio::{sync::mpsc, try_join};
use tracing::instrument;
use x509_cert::{
    Certificate,
    der::{EncodePem, pem::LineEnding},
};

use crate::{
    WebhookError, WebhookHandler, WebhookServer, constants::CONVERSION_WEBHOOK_HTTPS_PORT,
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
pub struct ConversionWebhookOptions {
    /// The bind address to bind the HTTPS server to.
    pub socket_addr: SocketAddr,

    /// The namespace the operator/webhook is running in.
    pub namespace: String,

    /// The name of the Kubernetes service which points to the operator/webhook.
    pub service_name: String,

    /// The field manager used to apply Kubernetes objects, typically the operator name, e.g.
    /// `airflow-operator`.
    pub field_manager: String,
}

/// A ready-to-use CRD conversion webhook server.
///
/// See [`ConversionWebhookServer::new()`] for usage examples.
pub struct ConversionWebhookServer {
    crds: Vec<CustomResourceDefinition>,
    options: ConversionWebhookOptions,
    router: Router,
    client: Client,
    maintain_crds: bool,
}

impl ConversionWebhookServer {
    /// Creates a new conversion webhook server, which expects POST requests being made to the
    /// `/convert/{crd name}` endpoint.
    ///
    /// You need to provide a few things for every CRD passed in via the `crds_and_handlers` argument:
    ///
    /// 1. The CRD
    /// 2. A conversion function to convert between CRD versions. Typically you would use the
    ///    the auto-generated `try_convert` function on CRD spec definition structs for this.
    /// 3. A [`kube::Client`] used to create/update the CRDs.
    /// 4. If the CRDs should be maintained automatically. Use `stackable_operator::cli::ProductOperatorRun::disable_crd_maintenance`
    /// for this.
    // # Because of https://github.com/rust-lang/cargo/issues/3475 we can not use a real link here
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
    ///     cli::ProductOperatorRun,
    /// };
    ///
    /// # async fn test() {
    /// // Things that should already be in you operator:
    /// const OPERATOR_NAME: &str = "product-operator";
    /// let client = Client::try_default().await.expect("failed to create Kubernetes client");
    /// let ProductOperatorRun {
    ///     operator_environment,
    ///     disable_crd_maintenance,
    ///     ..
    /// } = ProductOperatorRun::parse();
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
    ///     field_manager: OPERATOR_NAME.to_owned(),
    ///     namespace: operator_environment.operator_namespace,
    ///     service_name: operator_environment.operator_service_name,
    /// };
    ///
    /// // Construct the conversion webhook server
    /// let conversion_webhook = ConversionWebhookServer::new(
    ///     crds_and_handlers,
    ///     options,
    ///     client,
    ///     !disable_crd_maintenance,
    /// )
    /// .await
    /// .expect("failed to create ConversionWebhookServer");
    ///
    /// conversion_webhook.run().await.expect("failed to run ConversionWebhookServer");
    /// # }
    /// ```
    #[instrument(
        name = "create_conversion_webhook_server",
        skip(crds_and_handlers, client)
    )]
    pub async fn new<H>(
        crds_and_handlers: impl IntoIterator<Item = (CustomResourceDefinition, H)>,
        options: ConversionWebhookOptions,
        client: Client,
        maintain_crds: bool,
    ) -> Result<Self, ConversionWebhookError>
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

        Ok(Self {
            options,
            router,
            client,
            crds,
            maintain_crds,
        })
    }

    pub async fn run(self) -> Result<(), ConversionWebhookError> {
        tracing::info!("starting conversion webhook server");

        let Self {
            options,
            router,
            client,
            crds,
            maintain_crds,
        } = self;

        let ConversionWebhookOptions {
            socket_addr,
            field_manager,
            namespace: operator_namespace,
            service_name: operator_service_name,
        } = &options;

        // This is how Kubernetes calls us, so it decides about the naming.
        // AFAIK we can not influence this, so this is the only SAN entry needed.
        let subject_alterative_dns_name =
            format!("{operator_service_name}.{operator_namespace}.svc",);

        let webhook_options = WebhookOptions {
            subject_alterative_dns_names: vec![subject_alterative_dns_name],
            socket_addr: *socket_addr,
        };

        let (server, mut cert_rx) = WebhookServer::new(router, webhook_options)
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
            .context(ReceiveCertificateFromChannelSnafu)?;
        if maintain_crds {
            Self::reconcile_crds(
                &client,
                field_manager,
                &crds,
                operator_namespace,
                operator_service_name,
                current_cert,
            )
            .await
            .context(ReconcileCrdsSnafu)?;
        }

        if maintain_crds {
            try_join!(
                Self::run_webhook_server(server),
                Self::run_crd_reconciliation_loop(
                    cert_rx,
                    &client,
                    field_manager,
                    &crds,
                    operator_namespace,
                    operator_service_name,
                ),
            )?;
        } else {
            Self::run_webhook_server(server).await?;
        };

        Ok(())
    }

    async fn run_webhook_server(server: WebhookServer) -> Result<(), ConversionWebhookError> {
        server.run().await.context(RunWebhookServerSnafu)
    }

    async fn run_crd_reconciliation_loop(
        mut cert_rx: mpsc::Receiver<Certificate>,
        client: &Client,
        field_manager: &str,
        crds: &[CustomResourceDefinition],
        operator_namespace: &str,
        operator_service_name: &str,
    ) -> Result<(), ConversionWebhookError> {
        while let Some(current_cert) = cert_rx.recv().await {
            Self::reconcile_crds(
                client,
                field_manager,
                crds,
                operator_namespace,
                operator_service_name,
                current_cert,
            )
            .await
            .context(ReconcileCrdsSnafu)?;
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn reconcile_crds(
        client: &Client,
        field_manager: &str,
        crds: &[CustomResourceDefinition],
        operator_namespace: &str,
        operator_service_name: &str,
        current_cert: Certificate,
    ) -> Result<(), ConversionWebhookError> {
        tracing::info!(
            crds = ?crds.iter().map(CustomResourceDefinition::name_any).collect::<Vec<_>>(),
            "Reconciling CRDs"
        );
        let ca_bundle = current_cert
            .to_pem(LineEnding::LF)
            .context(ConvertCaToPemSnafu)?;

        let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());
        for mut crd in crds.iter().cloned() {
            let crd_name = crd.name_any();

            crd.spec.conversion = Some(CustomResourceConversion {
                strategy: "Webhook".to_string(),
                webhook: Some(WebhookConversion {
                    // conversionReviewVersions indicates what ConversionReview versions are understood/preferred by the webhook.
                    // The first version in the list understood by the API server is sent to the webhook.
                    // The webhook must respond with a ConversionReview object in the same version it received.
                    conversion_review_versions: vec!["v1".to_string()],
                    client_config: Some(WebhookClientConfig {
                        service: Some(ServiceReference {
                            name: operator_service_name.to_owned(),
                            namespace: operator_namespace.to_owned(),
                            path: Some(format!("/convert/{crd_name}")),
                            port: Some(CONVERSION_WEBHOOK_HTTPS_PORT.into()),
                        }),
                        ca_bundle: Some(ByteString(ca_bundle.as_bytes().to_vec())),
                        url: None,
                    }),
                }),
            });

            let patch = Patch::Apply(&crd);
            let patch_params = PatchParams::apply(field_manager);
            crd_api
                .patch(&crd_name, &patch_params, &patch)
                .await
                .with_context(|_| UpdateCrdSnafu {
                    crd_name: crd_name.to_string(),
                })?;
        }
        Ok(())
    }
}
