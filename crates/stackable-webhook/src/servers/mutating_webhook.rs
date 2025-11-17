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

use super::{WebhookServerImplementation, WebhookServerImplementationError};
use crate::WebhookOptions;

#[derive(Debug, Snafu)]
pub enum MutatingWebhookError {
    #[snafu(display("failed to patch MutatingWebhookConfiguration {vwc_name:?}"))]
    PatchMutatingWebhookConfiguration {
        source: kube::Error,
        vwc_name: String,
    },
}

/// As the webhook is typed with the Resource type `R`, it can only handle a single resource
/// mutation. Use multiple [`MutatingWebhookServer`] if you need to mutate multiple resource kinds.
pub struct MutatingWebhookServer<H, S, R> {
    mutating_webhook_configuration: MutatingWebhookConfiguration,
    handler: H,
    handler_state: Arc<S>,
    resource: PhantomData<R>,

    disable_mutating_webhook_configuration_maintenance: bool,
    client: Client,

    /// The field manager used when maintaining the CRDs.
    field_manager: String,
}

impl<H, S, R> MutatingWebhookServer<H, S, R> {
    pub fn new(
        mutating_webhook_configuration: MutatingWebhookConfiguration,
        handler: H,
        handler_state: Arc<S>,
        disable_mutating_webhook_configuration_maintenance: bool,
        client: Client,
        field_manager: String,
    ) -> Self {
        Self {
            mutating_webhook_configuration,
            handler,
            handler_state,
            resource: PhantomData,
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
impl<H, S, R, Fut> WebhookServerImplementation for MutatingWebhookServer<H, S, R>
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
        options: &WebhookOptions,
    ) -> Result<(), WebhookServerImplementationError> {
        if self.disable_mutating_webhook_configuration_maintenance {
            return Ok(());
        }

        let mut mutating_webhook_configuration = self.mutating_webhook_configuration.clone();
        let vwc_name = mutating_webhook_configuration.name_any();
        tracing::info!(
            k8s.MutatingWebhookConfiguration.name = vwc_name,
            "reconciling mutating webhook configurations"
        );

        for webhook in mutating_webhook_configuration.webhooks.iter_mut().flatten() {
            // TODO: Think is this is a bit excessive
            // assert!(
            //     webhook.failure_policy.is_some(),
            //     "Users of the mutating webhook need to make an explicit choice on the failure policy"
            // );
            assert_eq!(
                webhook.admission_review_versions,
                vec!["v1"],
                "We decide how we de-serialize the JSON and with that what AdmissionReview version we support (currently only v1)"
            );

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

        let vwc_api: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());
        // Other than with the CRDs we don't need to force-apply the MutatingWebhookConfiguration
        let patch = Patch::Apply(&mutating_webhook_configuration);
        let patch_params = PatchParams::apply(&self.field_manager);

        vwc_api
            .patch(&vwc_name, &patch_params, &patch)
            .await
            .with_context(|_| PatchMutatingWebhookConfigurationSnafu { vwc_name })?;

        Ok(())
    }
}
