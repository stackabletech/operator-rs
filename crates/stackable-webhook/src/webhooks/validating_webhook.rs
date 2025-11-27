use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use axum::{Json, Router, routing::post};
use k8s_openapi::{ByteString, api::admissionregistration::v1::ValidatingWebhookConfiguration};
use kube::{
    Api, Client, Resource, ResourceExt,
    api::{Patch, PatchParams},
    core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
};
use serde::{Serialize, de::DeserializeOwned};
use snafu::{ResultExt, Snafu};
use tracing::instrument;
use x509_cert::Certificate;

use super::{Webhook, WebhookError};
use crate::{WebhookServerOptions, webhooks::get_webhook_client_config};

#[derive(Debug, Snafu)]
pub enum ValidatingWebhookError {
    #[snafu(display("failed to patch ValidatingWebhookConfiguration {vwc_name:?}"))]
    PatchValidatingWebhookConfiguration {
        source: kube::Error,
        vwc_name: String,
    },
}

/// Validating webhook, which let's you intercept object creations/modification and allow or deny
/// the object.
///
/// As the webhook is typed with the Resource type `R`, it can only handle a single resource
/// validation. Use multiple [`ValidatingWebhook`] if you need to validate multiple resource kinds.
///
/// ### Example usage
///
/// TODO
pub struct ValidatingWebhook<H, S, R> {
    validating_webhook_configuration: ValidatingWebhookConfiguration,
    handler: H,
    handler_state: Arc<S>,
    _resource: PhantomData<R>,

    disable_validating_webhook_configuration_maintenance: bool,
    client: Client,

    /// The field manager used when maintaining the ValidatingWebhookConfigurations.
    field_manager: String,
}

impl<H, S, R> ValidatingWebhook<H, S, R> {
    /// All webhooks need to set the admissionReviewVersions to `["v1"]`, as this validating webhook
    /// only supports that version! A failure to do so will result in a panic.
    ///
    /// Your [`ValidatingWebhookConfiguration`] can contain 0..n webhooks, but it is recommended to
    /// only have a single entry in there, as the clientConfig of all entries will be set to the
    /// same service, port and HTTP path.
    pub fn new(
        validating_webhook_configuration: ValidatingWebhookConfiguration,
        handler: H,
        handler_state: Arc<S>,
        disable_validating_webhook_configuration_maintenance: bool,
        client: Client,
        field_manager: String,
    ) -> Self {
        for webhook in validating_webhook_configuration.webhooks.iter().flatten() {
            assert_eq!(
                webhook.admission_review_versions,
                vec!["v1"],
                "We decide how we de-serialize the JSON and with that what AdmissionReview version we support (currently only v1)"
            );
        }

        Self {
            validating_webhook_configuration,
            handler,
            handler_state,
            _resource: PhantomData,
            disable_validating_webhook_configuration_maintenance,
            client,
            field_manager,
        }
    }

    fn http_path(&self) -> String {
        let validating_webhook_configuration_name =
            self.validating_webhook_configuration.name_any();
        format!("/validate/{validating_webhook_configuration_name}")
    }
}

#[async_trait]
impl<H, S, R, Fut> Webhook for ValidatingWebhook<H, S, R>
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

    #[instrument(skip(self))]
    async fn handle_certificate_rotation(
        &mut self,
        _new_certificate: &Certificate,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        if self.disable_validating_webhook_configuration_maintenance {
            return Ok(());
        }

        let mut validating_webhook_configuration = self.validating_webhook_configuration.clone();
        let vwc_name = validating_webhook_configuration.name_any();
        tracing::info!(
            k8s.validatingwebhookconfiguration.name = vwc_name,
            "reconciling validating webhook configurations"
        );

        for webhook in validating_webhook_configuration
            .webhooks
            .iter_mut()
            .flatten()
        {
            // We know how we can be called (and with what certificate), so we can always set that
            webhook.client_config =
                get_webhook_client_config(options, new_ca_bundle.to_owned(), self.http_path());
        }

        let vwc_api: Api<ValidatingWebhookConfiguration> = Api::all(self.client.clone());
        // Other than with the CRDs we don't need to force-apply the ValidatingWebhookConfiguration
        let patch = Patch::Apply(&validating_webhook_configuration);
        let patch_params = PatchParams::apply(&self.field_manager);

        vwc_api
            .patch(&vwc_name, &patch_params, &patch)
            .await
            .with_context(|_| PatchValidatingWebhookConfigurationSnafu { vwc_name })?;

        Ok(())
    }
}
