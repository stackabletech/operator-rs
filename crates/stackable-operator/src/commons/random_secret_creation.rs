use std::collections::BTreeMap;

use base64::Engine;
use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Resource, ResourceExt, api::DeleteParams};
use rand::{RngCore, SeedableRng, rngs::StdRng};
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

    #[snafu(display("failed to delete random secret"))]
    DeleteRandomSecret { source: kube::Error },

    #[snafu(display("object is missing metadata to build owner reference"))]
    ObjectMissingMetadataForOwnerRef { source: crate::builder::meta::Error },
}

/// This function creates a random Secret if it doesn't already exist.
///
/// As this function generates random Secret contents, if the Secret already exists, it will *not*
/// be patched, as otherwise we would generate new Secret contents on every reconcile. Which would
/// in turn cause Pods restarts, which would cause reconciles ;)
///
/// However, there is one special handling needed:
///
/// We can't mark Secrets as immutable, as this caused problems, see <https://github.com/stackabletech/issues/issues/843>.
/// As Secrets have been created as immutable up to SDP release 26.3.0, we need to delete the, to be
/// able to re-create them as mutable. This function detects old (immutable) Secrets and re-creates
/// them as mutable. The contents of the Secret will be kept to prevent unnecessary Secret content
/// changes.
//
// TODO: This can be removed in a future SDP release, likely 26.11, as all Secrets have been migrated.
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

    match existing_secret {
        Some(
            existing_secret @ Secret {
                immutable: Some(true),
                ..
            },
        ) => {
            tracing::info!(
                k8s.secret.name = secret_name,
                k8s.secret.namespace = secret_namespace,
                "Old (immutable) random Secret detected, re-creating it to be able to make it mutable. The contents will stay the same."
            );
            Api::<Secret>::namespaced(client.as_kube_client(), &secret_namespace)
                .delete(secret_name, &DeleteParams::default())
                .await
                .context(DeleteRandomSecretSnafu)?;

            let mut mutable_secret = existing_secret;
            mutable_secret.immutable = Some(false);
            // Prevent "ApiError: resourceVersion should not be set on objects to be created"
            mutable_secret.metadata.resource_version = None;

            client
                .create(&mutable_secret)
                .await
                .context(CreateRandomSecretSnafu)?;

            // Note: restart-controller will restart all Pods mounting this Secret, as it has
            // changed.
        }
        Some(_) => {
            tracing::debug!(
                k8s.secret.name = secret_name,
                k8s.secret.namespace = secret_namespace,
                "Existing (mutable) random Secret detected, nothing to do"
            );
        }
        None => {
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
    }

    Ok(())
}

/// Generates a cryptographically secure base64 String with the specified size in bytes.
fn get_random_base64(size_bytes: usize) -> String {
    // As we are using the OS rng, we are using `getrandom`, which should be cryptographically
    // secure
    let mut rng = StdRng::from_os_rng();

    let mut bytes = vec![0u8; size_bytes];
    rng.fill_bytes(&mut bytes);

    base64::engine::general_purpose::STANDARD.encode(bytes)
}
