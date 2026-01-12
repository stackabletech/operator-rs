use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use axum::{Json, Router, routing::post};
use k8s_openapi::{ByteString, api::admissionregistration::v1::MutatingWebhookConfiguration};
use kube::{
    Api, Client, Resource, ResourceExt,
    api::{Patch, PatchParams},
    core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
};
use serde::{Serialize, de::DeserializeOwned};
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use super::{Webhook, WebhookError};
use crate::{WebhookServerOptions, webhooks::create_webhook_client_config};

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
/// use k8s_openapi::api::{
///     admissionregistration::v1::MutatingWebhookConfiguration, apps::v1::StatefulSet,
/// };
/// use stackable_operator::kube::{
///     Client,
///     core::admission::{AdmissionRequest, AdmissionResponse},
/// };
/// use stackable_webhook::{
///     WebhookServer,
///     webhooks::{MutatingWebhook, MutatingWebhookOptions},
/// };
///
/// # async fn docs() {
/// // The Kubernetes client
/// let client = Client::try_default().await.unwrap();
/// // The context of the controller, e.g. contains a Kubernetes client
/// let ctx = Arc::new(());
/// // Read in from user input, e.g. CLI arguments
/// let disable_mwc_maintenance = false;
///
/// let mutating_webhook_options = MutatingWebhookOptions {
///     disable_mwc_maintenance,
///     field_manager: "my-field-manager".to_owned(),
/// };
/// let mutating_webhook = Box::new(MutatingWebhook::new(
///     get_mutating_webhook_configuration(),
///     my_handler,
///     ctx,
///     client,
///     mutating_webhook_options,
/// ));
///
/// let webhook_options = todo!();
/// let webhook_server = WebhookServer::new(vec![mutating_webhook], webhook_options)
///     .await
///     .unwrap();
/// webhook_server.run().await.unwrap();
/// # }
///
/// fn get_mutating_webhook_configuration() -> MutatingWebhookConfiguration {
///     let webhook_name = "pod-labeler.stackable.tech";
///
///     MutatingWebhookConfiguration {
///         webhooks: Some(vec![
///             k8s_openapi::api::admissionregistration::v1::MutatingWebhook {
///                 // This is checked by the stackable_webhook code
///                 admission_review_versions: vec!["v1".to_owned()],
///                 ..Default::default()
///             },
///         ]),
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
    options: MutatingWebhookOptions,

    /// The [`MutatingWebhookConfiguration`] that is applied to the Kubernetes cluster.
    ///
    /// Your [`MutatingWebhookConfiguration`] can contain 0..n webhooks, but it is recommended to
    /// only have a single entry in there, as the clientConfig of all entries will be set to the
    /// same service, port and HTTP path.
    ///
    /// All webhooks need to set the `admissionReviewVersions` to `["v1"]`, as this mutating webhook
    /// only supports that version! A failure to do so will result in a panic during the
    /// [`MutatingWebhook`] creation.
    mutating_webhook_configuration: MutatingWebhookConfiguration,

    /// The async handler that get's a [`AdmissionRequest`] and returns an [`AdmissionResponse`]
    handler: H,

    /// The internal state of the webhook. You can define yourself what exactly this state is.
    handler_state: Arc<S>,

    /// The Kubernetes client used to maintain the MutatingWebhookConfigurations
    client: Client,

    /// This field is not needed, it only tracks the type of the Kubernetes resource we are mutating
    _resource: PhantomData<R>,
}

/// Configuration of a [`MutatingWebhook`], which is passed to [`MutatingWebhook::new`]
pub struct MutatingWebhookOptions {
    /// Whether MutatingWebhookConfigurations should be maintained
    pub disable_mwc_maintenance: bool,

    /// The field manager used when maintaining the MutatingWebhookConfigurations
    pub field_manager: String,
}

impl<H, S, R> MutatingWebhook<H, S, R> {
    pub fn new(
        mutating_webhook_configuration: MutatingWebhookConfiguration,
        handler: H,
        handler_state: Arc<S>,
        client: Client,
        options: MutatingWebhookOptions,
    ) -> Self {
        for webhook in mutating_webhook_configuration.webhooks.iter().flatten() {
            assert_eq!(
                webhook.admission_review_versions,
                vec!["v1"],
                "We decide how we de-serialize the JSON and with that what AdmissionReview version we support (currently only v1)"
            );
        }

        Self {
            options,
            mutating_webhook_configuration,
            handler,
            handler_state,
            _resource: PhantomData,
            client,
        }
    }

    fn http_path(&self) -> String {
        let mutating_webhook_configuration_name = self.mutating_webhook_configuration.name_any();
        format!("/mutate/{mutating_webhook_configuration_name}")
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
        tracing::debug!(route, "Registering route for mutating webhook");
        router.route(&route, post(handler_fn))
    }

    fn ignore_certificate_rotation(&self) -> bool {
        self.options.disable_mwc_maintenance
    }

    #[instrument(skip(self, new_ca_bundle))]
    async fn handle_certificate_rotation(
        &mut self,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError> {
        let mut mutating_webhook_configuration = self.mutating_webhook_configuration.clone();
        let mwc_name = mutating_webhook_configuration.name_any();
        tracing::info!(
            k8s.mutatingwebhookconfiguration.name = mwc_name,
            "reconciling mutating webhook configurations"
        );

        for webhook in mutating_webhook_configuration.webhooks.iter_mut().flatten() {
            // We know how we can be called (and with what certificate), so we can always set that
            webhook.client_config =
                create_webhook_client_config(options, new_ca_bundle.to_owned(), self.http_path());
        }

        let mwc_api: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());
        // Other than with the CRDs we don't need to force-apply the MutatingWebhookConfiguration
        // This is because the operators are, have been (and likely will be) the only ones creating
        // them.
        let patch = Patch::Apply(&mutating_webhook_configuration);
        let patch_params = PatchParams::apply(&self.options.field_manager);

        mwc_api
            .patch(&mwc_name, &patch_params, &patch)
            .await
            .with_context(|_| PatchMutatingWebhookConfigurationSnafu { mwc_name })?;

        Ok(())
    }
}
