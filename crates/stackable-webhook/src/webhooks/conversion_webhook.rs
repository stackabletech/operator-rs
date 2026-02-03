use std::fmt::Debug;

use async_trait::async_trait;
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
use snafu::{ResultExt, Snafu};
use tokio::sync::oneshot;
use tracing::instrument;

use crate::{Webhook, WebhookError, WebhookServerOptions};

#[derive(Debug, Snafu)]
pub enum ConversionWebhookError {
    #[snafu(display("failed to patch CRD {crd_name:?}"))]
    PatchCrd {
        source: kube::Error,
        crd_name: String,
    },
}

/// Conversion webhook, which converts between different versions of the same CRD.
///
/// ### Example usage
/// ```
/// use std::sync::Arc;
///
/// use stackable_operator::{
///     crd::s3::{S3Connection, S3ConnectionVersion},
///     kube::{
///         Client,
///         core::admission::{AdmissionRequest, AdmissionResponse},
///     },
/// };
/// use stackable_webhook::{
///     WebhookServer,
///     webhooks::{ConversionWebhook, ConversionWebhookOptions},
/// };
/// use tokio::time::{Duration, sleep};
///
/// # async fn docs() {
/// // The Kubernetes client
/// let client = Client::try_default().await.unwrap();
/// // Read in from user input, e.g. CLI arguments
/// let disable_crd_maintenance = false;
///
/// let crds_and_handlers = vec![(
///     S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1)
///         .expect("the S3Connection CRD must be merged"),
///     S3Connection::try_convert,
/// )];
///
/// let conversion_webhook_options = ConversionWebhookOptions {
///     disable_crd_maintenance,
///     field_manager: "my-field-manager".to_owned(),
/// };
/// let (conversion_webhook, initial_reconcile_rx) =
///     ConversionWebhook::new(crds_and_handlers, client, conversion_webhook_options);
///
/// let webhook_options = todo!();
/// let webhook_server = WebhookServer::new(vec![Box::new(conversion_webhook)], webhook_options)
///     .await
///     .unwrap();
/// let shutdown_signal = sleep(Duration::from_millis(100));
///
/// webhook_server.run(shutdown_signal).await.unwrap();
/// # }
/// ```
pub struct ConversionWebhook<H> {
    options: ConversionWebhookOptions,

    /// The list of 2-tuple (pair) mapping a [`CustomResourceDefinition`] to a [`ConversionReview`]
    /// handler function. In most cases, the generated `CustomResource::try_merge` function should
    /// be used. It provides the expected `fn(ConversionReview) -> ConversionReview` signature.
    crds_and_handlers: Vec<(CustomResourceDefinition, H)>,

    /// The Kubernetes client used to maintain the CRDs
    client: Client,

    /// The values is send as soon as all CRDs have been applied to the cluster
    // This channel can only be used exactly once. The sender's send method consumes self, and
    // as such, the sender is wrapped in an Option to be able to call take to consume the inner
    // value.
    initial_reconcile_tx: Option<oneshot::Sender<()>>,
}

/// Configuration of a [`ConversionWebhook`], which is passed to [`ConversionWebhook::new`]
pub struct ConversionWebhookOptions {
    /// Whether CRDs should be maintained
    pub disable_crd_maintenance: bool,

    /// The field manager used when maintaining the CRDs
    pub field_manager: String,
}

impl<H> ConversionWebhook<H> {
    /// Creates a new [`ConversionWebhook`] with the given list of CRDs and handlers converting
    /// between different versions of them.
    ///
    /// ## Return Values
    ///
    /// This function returns a 2-tuple (pair) of values:
    ///
    /// - The new [`ConversionWebhook`] itself
    /// - The [`oneshot::Receiver`] that informs the caller that the CRDs have been reconciled
    ///   initially. This guarantees that the CRDs are now install on the Kubernetes cluster and the
    ///   caller can apply CustomResources of that kind.
    pub fn new(
        crds_and_handlers: Vec<(CustomResourceDefinition, H)>,
        client: Client,
        options: ConversionWebhookOptions,
    ) -> (Self, oneshot::Receiver<()>) {
        let (initial_reconcile_tx, initial_reconcile_rx) = oneshot::channel();

        let new = Self {
            options,
            crds_and_handlers,
            client,
            initial_reconcile_tx: Some(initial_reconcile_tx),
        };

        (new, initial_reconcile_rx)
    }

    #[instrument(
        skip(self, crd, crd_api, ca_bundle),
        fields(
            name = crd.name_any(),
            kind = &crd.spec.names.kind
        )
    )]
    async fn reconcile_crd(
        &self,
        mut crd: CustomResourceDefinition,
        crd_api: &Api<CustomResourceDefinition>,
        ca_bundle: ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        let crd_kind = &crd.spec.names.kind;
        let crd_name = crd.name_any();

        tracing::info!(
            k8s.crd.kind = crd_kind,
            k8s.crd.name = crd_name,
            "reconciling custom resource definition"
        );

        crd.spec.conversion = Some(CustomResourceConversion {
            strategy: "Webhook".to_owned(),
            webhook: Some(WebhookConversion {
                // conversionReviewVersions indicates what ConversionReview versions are
                // supported by the webhook. The first version in the list understood by the
                // API server is sent to the webhook. The webhook must respond with a
                // ConversionReview object in the same version it received. We only support
                // the stable v1 ConversionReview to keep the implementation as simple as
                // possible.
                conversion_review_versions: vec!["v1".to_owned()],
                client_config: Some(WebhookClientConfig {
                    service: Some(ServiceReference {
                        name: options.webhook_service_name.to_owned(),
                        namespace: options.webhook_namespace.to_owned(),
                        path: Some(format!("/convert/{crd_name}")),
                        port: Some(options.socket_addr.port().into()),
                    }),
                    // Here, ByteString takes care of encoding the provided content as base64.
                    ca_bundle: Some(ca_bundle),
                    url: None,
                }),
            }),
        });

        // Deploy the updated CRDs using a server-side apply.
        let patch = Patch::Apply(&crd);

        // We force apply here, because we want to become the sole manager of the CRD. This
        // avoids any conflicts from previous deployments via helm or stackablectl which are
        // reported with the following error message:
        //
        // Apply failed with 2 conflicts: conflicts with "stackablectl" using apiextensions.k8s.io/v1:
        //   - .spec.versions
        //   - .spec.conversion.strategy: Conflict
        //
        // The official Kubernetes documentation provides three options on how to solve
        // these conflicts. Option 1 is used, which is described as follows:
        //
        // Overwrite value, become sole manager: If overwriting the value was intentional
        // (or if the applier is an automated process like a controller) the applier should
        // set the force query parameter to true [...], and make the request again. This
        // forces the operation to succeed, changes the value of the field, and removes the
        // field from all other managers' entries in managedFields.
        //
        // See https://kubernetes.io/docs/reference/using-api/server-side-apply/#conflicts
        let patch_params = PatchParams::apply(&self.options.field_manager).force();

        crd_api
            .patch(&crd_name, &patch_params, &patch)
            .await
            .with_context(|_| PatchCrdSnafu { crd_name })?;

        Ok(())
    }
}

#[async_trait]
impl<H> Webhook for ConversionWebhook<H>
where
    H: FnOnce(ConversionReview) -> ConversionReview + Clone + Send + Sync + 'static,
{
    fn register_routes(&self, mut router: Router) -> Router {
        for (crd, handler) in &self.crds_and_handlers {
            let handler = handler.clone();
            let crd_name = crd.name_any();
            let handler_fn = |Json(review): Json<ConversionReview>| async {
                let review = handler(review);
                Json(review)
            };

            let route = format!("/convert/{crd_name}");
            tracing::debug!(
                k8s.crd.kind = &crd.spec.names.kind,
                k8s.crd.name = crd_name,
                route,
                "Registering route for conversion webhook"
            );
            router = router.route(&route, post(handler_fn));
        }

        router
    }

    fn ignore_certificate_rotation(&self) -> bool {
        self.options.disable_crd_maintenance
    }

    #[instrument(skip(self, ca_bundle))]
    async fn handle_certificate_rotation(
        &mut self,
        ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        for (crd, _) in &self.crds_and_handlers {
            self.reconcile_crd(crd.clone(), &crd_api, ca_bundle.to_owned(), options)
                .await?;
        }

        // After the reconciliation of the CRDs, the initial reconcile heartbeat is sent out
        // via the oneshot channel.
        if let Some(initial_reconcile_tx) = self.initial_reconcile_tx.take() {
            // This call will (only) error in case the receiver is dropped, so we need to ignore
            // failures.
            let _ = initial_reconcile_tx.send(());
        }

        Ok(())
    }
}
