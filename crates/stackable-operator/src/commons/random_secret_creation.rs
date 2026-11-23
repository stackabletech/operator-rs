use std::collections::BTreeMap;

use base64::Engine;
use k8s_openapi::api::core::v1::Secret;
use kube::{Resource, ResourceExt};
use rand::{Rng, rngs::StdRng};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{builder::meta::ObjectMetaBuilder, client::Client};

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("object defines no namespace"))]
    ObjectHasNoNamespace,

    #[snafu(display("failed to retrieve random secret"))]
    RetrieveRandomSecret { source: crate::client::Error },

    #[snafu(display("failed to create random secret"))]
    CreateRandomSecret { source: crate::client::Error },

    #[snafu(display("object is missing metadata to build owner reference"))]
    ObjectMissingMetadataForOwnerRef { source: crate::builder::meta::Error },
}

/// This function creates a random Secret if it doesn't already exist.
///
/// As this function generates random Secret contents, if the Secret already exists, it will *not*
/// be patched, as otherwise we would generate new Secret contents on every reconcile. Which would
/// in turn cause Pods restarts, which would cause reconciles ;)
pub async fn create_random_secret_if_not_exists<R>(
    secret_name: &str,
    secret_key: &str,
    secret_size_bytes: usize,
    stacklet: &R,
    client: &Client,
) -> Result<(), Error>
where
    R: Resource<DynamicType = ()>,
{
    let secret_namespace = stacklet.namespace().context(ObjectHasNoNamespaceSnafu)?;
    let existing_secret = client
        .get_opt::<Secret>(secret_name, &secret_namespace)
        .await
        .context(RetrieveRandomSecretSnafu)?;

    if existing_secret.is_some() {
        tracing::debug!(
            k8s.secret.name = secret_name,
            k8s.secret.namespace = secret_namespace,
            "Existing random Secret detected, nothing to do"
        );
    } else {
        tracing::info!(
            k8s.secret.name = secret_name,
            k8s.secret.namespace = secret_namespace,
            "Random Secret missing, creating it"
        );
        let secret = Secret {
            metadata: ObjectMetaBuilder::new()
                .name(secret_name)
                .namespace_opt(stacklet.namespace())
                .ownerreference_from_resource(stacklet, None, Some(true))
                .context(ObjectMissingMetadataForOwnerRefSnafu)?
                .build(),
            string_data: Some(BTreeMap::from([(
                secret_key.to_string(),
                get_random_base64(secret_size_bytes),
            )])),
            ..Secret::default()
        };
        client
            .create(&secret)
            .await
            .context(CreateRandomSecretSnafu)?;
    }

    Ok(())
}

/// Generates a cryptographically secure base64 String with the specified size in bytes.
fn get_random_base64(size_bytes: usize) -> String {
    // As `make_rng` takes its entropy from the OS rng, we are using `getrandom`, which should be
    // cryptographically secure
    let mut rng: StdRng = rand::make_rng();

    let mut bytes = vec![0u8; size_bytes];
    rng.fill_bytes(&mut bytes);

    base64::engine::general_purpose::STANDARD.encode(bytes)
}
