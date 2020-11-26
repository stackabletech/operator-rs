use crate::error::Error;
use kube::api::{Meta, PatchParams, PatchStrategy};
use kube::{Api, Client};
use serde_json::json;
use serde::de::DeserializeOwned;


/// Checks whether our own finalizer is in the list of finalizers for the provided object.
pub fn has_finalizer<T>(resource: &T, finalizer: &str) -> bool
    where T: Meta
{
    return match resource.meta().finalizers.as_ref() {
        Some(finalizers) => finalizers.contains(&finalizer.to_string()),
        None => false,
    };
}

/// Adds our finalizer to the list of finalizers.
pub async fn add_finalizer<T>(
    client: Client,
    resource: &T,
    finalizer: &str
) -> Result<T, Error>
    where T: k8s_openapi::Resource + Clone + Meta + DeserializeOwned
{
    let name = &Meta::name(resource);
    let namespace = &Meta::namespace(resource).expect("Resource is namespaced");

    let api: Api<T> = Api::namespaced(client, namespace);
    let new_metadata = serde_json::to_vec(&json!({
        "metadata": {
            "finalizers": [finalizer.to_string()]
        }
    }))?;

    let patch_params = PatchParams {
        patch_strategy: PatchStrategy::Merge,
        field_manager: None, // TODO?
        ..PatchParams::default()
    };

    api
        .patch(name, &patch_params, new_metadata)
        .await
        .map_err(Error::from)
}

/// Removes our finalizer from a resource object.
///
/// # Arguments
/// `name` - is the name of the resource we want to patch
/// `namespace` is the namespace of where the resource to patch lives
pub async fn remove_finalizer<T>(client: Client, resource: &T, finalizer: &str) -> Result<T, Error>
    where T: Clone + DeserializeOwned + Meta
{
    // It would be preferable to use a strategic merge but that currently (K8S 1.19) doesn't
    // seem to work against custom resources.
    // This is what the patch could look like
    // ```
    //         "metadata": {
    //             "$deleteFromPrimitiveList/finalizers": [FINALIZER_NAME.to_string()]
    //         }
    // ```
    let name = Meta::name(resource);
    let namespace = Meta::namespace(resource).expect("Custom Resource is namespaced");
    let api: Api<T> = Api::namespaced(client, &namespace);

    return match resource.meta().finalizers.clone() {
        None => Err(Error::MissingObjectKey {
            key: ".metadata.finalizers",
        }),
        Some(mut finalizers) => {
            let index = finalizers
                .iter()
                .position(|cur_finalizer| cur_finalizer == finalizer);

            if let Some(index) = index {
                // We found our finalizer which means that we now need to handle our deletion logic
                // And then remove the finalizer from the list.

                finalizers.swap_remove(index);
                let new_metadata = serde_json::to_vec(&json!({
                    "metadata": {
                        "finalizers": finalizers
                    }
                }))?;

                let patch_params = PatchParams {
                    patch_strategy: PatchStrategy::Merge,
                    field_manager: None, // TODO?
                    ..PatchParams::default()
                };

                api
                    .patch(&name, &patch_params, new_metadata)
                    .await
                    .map_err(Error::from)
            } else {
                Err(Error::MissingObjectKey {
                    key: ".metadata.finalizers",
                })
            }
        }
    };
}

/// Checks whether the provided object has a deletion timestamp set.
/// If that is the case the object is in the process of being deleted pending the handling of all finalizers.
pub fn has_deletion_stamp<T>(obj: &T) -> bool
    where T: Meta
{
    return obj.meta().deletion_timestamp.is_some();
}
