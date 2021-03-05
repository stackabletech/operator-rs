use crate::client::Client;
use crate::error::{Error, OperatorResult};

use kube::api::Meta;
use serde::de::DeserializeOwned;
use serde_json::json;

/// Checks whether our own finalizer is in the list of finalizers for the provided object.
pub fn has_finalizer<T>(resource: &T, finalizer: &str) -> bool
where
    T: Meta,
{
    return match resource.meta().finalizers.as_ref() {
        Some(finalizers) => finalizers.contains(&finalizer.to_string()),
        None => false,
    };
}

/// This will add the passed finalizer to the list of finalizers for the resource and will
/// update the resource in Kubernetes.
pub async fn add_finalizer<T>(client: &Client, resource: &T, finalizer: &str) -> OperatorResult<T>
where
    T: Clone + Meta + DeserializeOwned,
{
    let new_metadata = json!({
        "metadata": {
            "finalizers": [finalizer.to_string()]
        }
    });
    client.merge_patch(resource, new_metadata).await
}

/// Removes our finalizer from a resource object.
///
/// # Arguments
/// `name` - is the name of the resource we want to patch
/// `namespace` is the namespace of where the resource to patch lives
pub async fn remove_finalizer<T>(client: Client, resource: &T, finalizer: &str) -> OperatorResult<T>
where
    T: Clone + DeserializeOwned + Meta,
{
    // It would be preferable to use a strategic merge but that currently (K8S 1.19) doesn't
    // seem to work against custom resources.
    // This is what the patch could look like
    // ```
    //         "metadata": {
    //             "$deleteFromPrimitiveList/finalizers": [FINALIZER_NAME.to_string()]
    //         }
    // ```
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
                let new_metadata = json!({
                    "metadata": {
                        "finalizers": finalizers
                    }
                });

                client.merge_patch(resource, new_metadata).await
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
where
    T: Meta,
{
    return obj.meta().deletion_timestamp.is_some();
}
