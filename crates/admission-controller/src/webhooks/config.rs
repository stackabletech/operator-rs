use std::marker::PhantomData;

use k8s_openapi::api::admissionregistration::v1::{
    MutatingWebhook, MutatingWebhookConfiguration, ValidatingWebhook,
    ValidatingWebhookConfiguration,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::Resource;

pub struct WebhookConfiguration;

impl WebhookConfiguration {
    /// Create a validating webhook configuration
    pub fn validating(
        name: impl Into<String>,
    ) -> WebhookConfigurationBuilder<ValidatingWebhook, ValidatingWebhookConfiguration> {
        // TODO (Techassi): Add name validation. Needs to be DNS name
        let config = ValidatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            ..Default::default()
        };

        WebhookConfigurationBuilder {
            hooks: PhantomData,
            config,
        }
    }

    /// Create a mutating webhook configuration
    pub fn mutating(
        name: impl Into<String>,
    ) -> WebhookConfigurationBuilder<MutatingWebhook, MutatingWebhookConfiguration> {
        // TODO (Techassi): Add name validation. Needs to be DNS name
        let config = MutatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            ..Default::default()
        };

        WebhookConfigurationBuilder {
            hooks: PhantomData,
            config,
        }
    }
}

pub trait WebhookConfigurationExt<H> {
    fn webhooks_mut(&mut self) -> &mut Vec<H>;
}

impl WebhookConfigurationExt<ValidatingWebhook> for ValidatingWebhookConfiguration {
    fn webhooks_mut(&mut self) -> &mut Vec<ValidatingWebhook> {
        self.webhooks.get_or_insert(Vec::new())
    }
}

impl WebhookConfigurationExt<MutatingWebhook> for MutatingWebhookConfiguration {
    fn webhooks_mut(&mut self) -> &mut Vec<MutatingWebhook> {
        self.webhooks.get_or_insert(Vec::new())
    }
}

/// The [`WebhookConfigurationBuilder`] helps to create valid admission webhook
/// configurations. Webhooks can either be [validating][k8s-val] or
/// [mutating][k8s-mut].
///
/// [k8s-val]: https://kubernetes.io/docs/reference/access-authn-authz/admission-controllers/#validatingadmissionwebhook
/// [k8s-mut]: https://kubernetes.io/docs/reference/access-authn-authz/admission-controllers/#mutatingadmissionwebhook
#[derive(Debug, Default)]
pub struct WebhookConfigurationBuilder<H, C>
where
    C: Resource + WebhookConfigurationExt<H>,
{
    hooks: PhantomData<H>,
    config: C,
}

impl<H, C> WebhookConfigurationBuilder<H, C>
where
    C: Resource + WebhookConfigurationExt<H>,
{
    pub fn add_webhook(&mut self, webhook: H) -> &mut Self {
        self.config.webhooks_mut().push(webhook);
        self
    }

    pub fn build(self) -> C {
        self.config
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn generics() {
        let mut builder = WebhookConfiguration::validating("name");
        builder.add_webhook(ValidatingWebhook::default());

        let config = builder.build();
        println!("{:?}", config)
    }
}
