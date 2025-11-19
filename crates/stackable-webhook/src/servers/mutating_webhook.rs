use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use axum::{Json, Router, routing::post};
use k8s_openapi::{
    ByteString,
    api::admissionregistration::v1::{
        MutatingWebhookConfiguration, ServiceReference, WebhookClientConfig,
    },
};
use kube::{
    Api, Client, Resource, ResourceExt,
    api::{Patch, PatchParams},
    core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
};
use serde::{Serialize, de::DeserializeOwned};
use snafu::{ResultExt, Snafu};
use x509_cert::Certificate;

use super::{Webhook, WebhookError};
use crate::WebhookServerOptions;

#[derive(Debug, Snafu)]
pub enum MutatingWebhookError {
    #[snafu(display("failed to patch MutatingWebhookConfiguration {mwc_name:?}"))]
    PatchMutatingWebhookConfiguration {
        source: kube::Error,
        mwc_name: String,
    },
}

/// Mutating webhook, which let's you intercept object creations/modification and modify the object
/// on the fly.
///
/// As the webhook is typed with the Resource type `R`, it can only handle a single resource
/// mutation. Use multiple [`MutatingWebhook`] if you need to mutate multiple resource kinds.
///
/// ### Example usage
///
/// This is only some high-level basic usage!
///
/// For concrete usage please have a look at the restart controller mutating webhook in
/// commons-operator.
///
/// ```
/// use std::sync::Arc;
///
/// use k8s_openapi::api::admissionregistration::v1::MutatingWebhookConfiguration;
/// use k8s_openapi::api::apps::v1::StatefulSet;
///
/// use stackable_operator::builder::meta::ObjectMetaBuilder;
/// use stackable_operator::kube::Client;
/// use stackable_operator::kube::core::admission::{AdmissionRequest, AdmissionResponse};
/// use stackable_operator::kvp::Label;
/// use stackable_webhook::WebhookServer;
/// use stackable_webhook::servers::MutatingWebhook;
///
/// # async fn docs() {
/// // The Kubernetes client
/// let client = Client::try_default().await.unwrap();
/// // The context of the controller, e.g. contains a Kubernetes client
/// let ctx = Arc::new(());
/// // Read in from user input, e.g. CLI arguments
/// let disable_restarter_mutating_webhook = false;
///
/// let mutating_webhook = Box::new(MutatingWebhook::new(
///     get_mutating_webhook_configuration(),
///     my_handler,
///     ctx,
///     disable_restarter_mutating_webhook,
///     client,
///     "my-field-manager".to_owned(),
/// ));
///
/// let webhook_options = todo!();
/// let webhook_server = WebhookServer::new(webhook_options, vec![mutating_webhook]).await.unwrap();
/// webhook_server.run().await.unwrap();
/// # }
///
/// fn get_mutating_webhook_configuration() -> MutatingWebhookConfiguration {
///     let webhook_name = "pod-labeler.stackable.tech";
///
///     MutatingWebhookConfiguration {
///         webhooks: Some(vec![k8s_openapi::api::admissionregistration::v1::MutatingWebhook {
///             // This is checked by the stackable_webhook code
///             admission_review_versions: vec!["v1".to_owned()],
///             ..Default::default()
///         }]),
///         ..Default::default()
///     }
/// }
///
/// // Basic no-op implementation
/// pub async fn my_handler(
///     ctx: Arc<()>,
///     request: AdmissionRequest<StatefulSet>,
/// ) -> AdmissionResponse {
///     AdmissionResponse::from(&request)
/// }
/// ```
pub struct MutatingWebhook<H, S, R> {
    mutating_webhook_configuration: MutatingWebhookConfiguration,
    handler: H,
    handler_state: Arc<S>,
    _resource: PhantomData<R>,

    disable_mutating_webhook_configuration_maintenance: bool,
    client: Client,

    /// The field manager used when maintaining the CRDs.
    field_manager: String,
}

impl<H, S, R> MutatingWebhook<H, S, R> {
    /// All webhooks need to set the admissionReviewVersions to `["v1"]`, as this mutating webhook
    /// only supports that version! A failure to do so will result in a panic.
    pub fn new(
        mutating_webhook_configuration: MutatingWebhookConfiguration,
        handler: H,
        handler_state: Arc<S>,
        disable_mutating_webhook_configuration_maintenance: bool,
        client: Client,
        field_manager: String,
    ) -> Self {
        for webhook in mutating_webhook_configuration.webhooks.iter().flatten() {
            assert_eq!(
                webhook.admission_review_versions,
                vec!["v1"],
                "We decide how we de-serialize the JSON and with that what AdmissionReview version we support (currently only v1)"
            );
        }

        Self {
            mutating_webhook_configuration,
            handler,
            handler_state,
            _resource: PhantomData,
            disable_mutating_webhook_configuration_maintenance,
            client,
            field_manager,
        }
    }

    fn http_path(&self) -> String {
        format!("/mutate/{}", self.mutating_webhook_configuration.name_any())
    }
}

#[async_trait]
impl<H, S, R, Fut> Webhook for MutatingWebhook<H, S, R>
where
    H: Fn(Arc<S>, AdmissionRequest<R>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = AdmissionResponse> + Send + 'static,
    R: Resource + Send + Sync + DeserializeOwned + Serialize + 'static,
    S: Send + Sync + 'static,
{
    fn register_routes(&self, router: Router) -> Router {
        let handler_state = self.handler_state.clone();
        let handler = self.handler.clone();
        let handler_fn = |Json(review): Json<AdmissionReview<R>>| async move {
            let request: AdmissionRequest<R> = match review.try_into() {
                Ok(request) => request,
                Err(err) => {
                    return Json(
                        AdmissionResponse::invalid(format!("failed to convert to request: {err}"))
                            .into_review(),
                    );
                }
            };

            let response = handler(handler_state, request).await;
            let review = response.into_review();
            Json(review)
        };

        let route = self.http_path();
        router.route(&route, post(handler_fn))
    }

    async fn handle_certificate_rotation(
        &mut self,
        _new_certificate: &Certificate,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        if self.disable_mutating_webhook_configuration_maintenance {
            return Ok(());
        }

        let mut mutating_webhook_configuration = self.mutating_webhook_configuration.clone();
        let mwc_name = mutating_webhook_configuration.name_any();
        tracing::info!(
            k8s.mutatingwebhookconfiguration.name = mwc_name,
            "reconciling mutating webhook configurations"
        );

        for webhook in mutating_webhook_configuration.webhooks.iter_mut().flatten() {
            // We know how we can be called (and with what certificate), so we can always set that
            webhook.client_config = WebhookClientConfig {
                service: Some(ServiceReference {
                    name: options.operator_service_name.to_owned(),
                    namespace: options.operator_namespace.to_owned(),
                    path: Some(self.http_path()),
                    port: Some(options.socket_addr.port().into()),
                }),
                // Here, ByteString takes care of encoding the provided content as base64.
                ca_bundle: Some(new_ca_bundle.to_owned()),
                url: None,
            };
        }

        let mwc_api: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());
        // Other than with the CRDs we don't need to force-apply the MutatingWebhookConfiguration
        let patch = Patch::Apply(&mutating_webhook_configuration);
        let patch_params = PatchParams::apply(&self.field_manager);

        mwc_api
            .patch(&mwc_name, &patch_params, &patch)
            .await
            .with_context(|_| PatchMutatingWebhookConfigurationSnafu { mwc_name })?;

        Ok(())
    }
}
