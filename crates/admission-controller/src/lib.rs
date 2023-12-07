use axum::{routing::post, Router};
use k8s_openapi::{
    api::admissionregistration::v1::{
        MutatingWebhook, MutatingWebhookConfiguration, ValidatingWebhook,
        ValidatingWebhookConfiguration,
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};

pub struct AdmissionController {
    router: Router,
}

impl AdmissionController {
    pub fn new() -> Self {
        let router = Router::new().route("/", post(|| async {}));

        Self { router }
    }

    pub async fn run(self) {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, self.router).await.unwrap();
    }
}

/// The [`WebhookConfigurationBuilder`] helps to create valid admission webhook
/// configurations. Webhooks can either be [validating][k8s-val] or
/// [mutating][k8s-mut].
///
/// [k8s-val]: https://kubernetes.io/docs/reference/access-authn-authz/admission-controllers/#validatingadmissionwebhook
/// [k8s-mut]: https://kubernetes.io/docs/reference/access-authn-authz/admission-controllers/#mutatingadmissionwebhook
#[derive(Debug, Default)]
pub struct WebhookConfigurationBuilder;

impl WebhookConfigurationBuilder {
    /// Create a validating webhook configuration
    pub fn validating(name: String) -> ValidatingWebhookConfigurationBuilder {
        // TODO (Techassi): Add name validation. Needs to be DNS name
        ValidatingWebhookConfigurationBuilder {
            webhooks: Vec::new(),
            name,
        }
    }

    /// Create a mutating webhook configuration
    pub fn mutating(name: String) -> MutatingWebhookConfigurationBuilder {
        // TODO (Techassi): Add name validation. Needs to be DNS name
        MutatingWebhookConfigurationBuilder {
            webhooks: Vec::new(),
            name,
        }
    }
}

pub struct ValidatingWebhookConfigurationBuilder {
    webhooks: Vec<ValidatingWebhook>,
    name: String,
}

impl ValidatingWebhookConfigurationBuilder {
    pub fn build(self) -> ValidatingWebhookConfiguration {
        ValidatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some(self.name),
                ..Default::default()
            },
            webhooks: (!self.webhooks.is_empty()).then_some(self.webhooks),
        }
    }
}

pub struct MutatingWebhookConfigurationBuilder {
    webhooks: Vec<MutatingWebhook>,
    name: String,
}

impl MutatingWebhookConfigurationBuilder {
    pub fn build(self) -> MutatingWebhookConfiguration {
        MutatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some(self.name),
                ..Default::default()
            },
            webhooks: (!self.webhooks.is_empty()).then_some(self.webhooks),
        }
    }
}
