use k8s_openapi::api::admissionregistration::v1::ValidatingWebhook;

use crate::webhooks::SideEffects;

pub trait ValidatingWebhookExt {
    fn builder(name: impl Into<String>, side_effects: SideEffects) -> ValidatingWebhookBuilder;
}

impl ValidatingWebhookExt for ValidatingWebhook {
    fn builder(name: impl Into<String>, side_effects: SideEffects) -> ValidatingWebhookBuilder {
        ValidatingWebhookBuilder::new(name.into(), side_effects)
    }
}

pub struct ValidatingWebhookBuilder {
    side_effects: SideEffects,
    name: String,
}

impl ValidatingWebhookBuilder {
    pub fn new(name: String, side_effects: SideEffects) -> Self {
        Self { side_effects, name }
    }

    pub fn build(self) -> ValidatingWebhook {
        ValidatingWebhook {
            side_effects: self.side_effects.to_string(),
            name: self.name,
            ..Default::default()
        }
    }
}
