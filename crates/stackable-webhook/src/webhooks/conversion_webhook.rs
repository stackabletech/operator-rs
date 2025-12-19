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
use snafu::{ResultExt, Snafu, ensure};
use tokio::sync::oneshot;
use tracing::instrument;
use x509_cert::Certificate;

use super::{Webhook, WebhookError};
use crate::WebhookServerOptions;

#[derive(Debug, Snafu)]
pub enum ConversionWebhookError {
    #[snafu(display("failed to send initial CRD reconcile heartbeat"))]
    SendInitialReconcileHeartbeat,

    #[snafu(display("failed to patch CRD {crd_name:?}"))]
    PatchCrd {
        source: kube::Error,
        crd_name: String,
    },
}

pub struct ConversionWebhook<H> {
    /// The list of CRDs and their according handlers, which take and return a [`ConversionReview`]
    crds_and_handlers: Vec<(CustomResourceDefinition, H)>,

    /// Whether CRDs should be maintained
    disable_crd_maintenance: bool,

    /// The Kubernetes client used to maintain the CRDs
    client: Client,

    /// The field manager used when maintaining the CRDs
    field_manager: String,

    // This channel can only be used exactly once. The sender's send method consumes self, and
    // as such, the sender is wrapped in an Option to be able to call take to consume the inner
    // value.
    initial_reconcile_tx: Option<oneshot::Sender<()>>,
}

impl<H> ConversionWebhook<H> {
    pub fn new(
        crds_and_handlers: impl IntoIterator<Item = (CustomResourceDefinition, H)>,
        disable_crd_maintenance: bool,
        client: Client,
        field_manager: String,
    ) -> (Self, oneshot::Receiver<()>) {
        let (initial_reconcile_tx, initial_reconcile_rx) = oneshot::channel();

        let new = Self {
            crds_and_handlers: crds_and_handlers.into_iter().collect(),
            disable_crd_maintenance,
            client,
            field_manager,
            initial_reconcile_tx: Some(initial_reconcile_tx),
        };

        (new, initial_reconcile_rx)
    }

    #[instrument(
        skip(self, crd, crd_api),
        fields(
            name = crd.name_any(),
            kind = &crd.spec.names.kind
        )
    )]
    async fn reconcile_crd(
        &self,
        mut crd: CustomResourceDefinition,
        crd_api: &Api<CustomResourceDefinition>,
        new_ca_bundle: &ByteString,
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
                    ca_bundle: Some(new_ca_bundle.to_owned()),
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
        let patch_params = PatchParams::apply(&self.field_manager).force();

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
                crd.name = crd_name,
                route,
                "Registering route for conversion webhook"
            );
            router = router.route(&route, post(handler_fn));
        }

        router
    }

    fn ignore_certificate_rotation(&self) -> bool {
        self.disable_crd_maintenance
    }

    #[instrument(skip(self))]
    async fn handle_certificate_rotation(
        &mut self,
        _new_certificate: &Certificate,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        for (crd, _) in &self.crds_and_handlers {
            self.reconcile_crd(crd.clone(), &crd_api, new_ca_bundle, options)
                .await?;
        }

        // After the reconciliation of the CRDs, the initial reconcile heartbeat is sent out
        // via the oneshot channel.
        if let Some(initial_reconcile_tx) = self.initial_reconcile_tx.take() {
            ensure!(
                initial_reconcile_tx.send(()).is_ok(),
                SendInitialReconcileHeartbeatSnafu
            );
        }

        Ok(())
    }
}
