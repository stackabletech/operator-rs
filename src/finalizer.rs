use crate::client::Client;
use crate::error::{Error, OperatorResult};

use json_patch::{PatchOperation, RemoveOperation, TestOperation};
use kube::api::Meta;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fmt::Debug;
use tracing::debug;

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

/// This will add the passed finalizer to the list of finalizers for the resource if it doesn't exist yet
/// and will update the resource in Kubernetes.
///
/// It'll return `true` if we changed the object in Kubernetes and `false` if no modification was needed.
/// If the object is currently being deleted this _will_ return an Error!
pub async fn add_finalizer<T>(
    client: &Client,
    resource: &T,
    finalizer: &str,
) -> OperatorResult<bool>
where
    T: Clone + Debug + Meta + DeserializeOwned,
    <T as Meta>::DynamicType: Default,
{
    if has_finalizer(resource, finalizer) {
        debug!("Finalizer [{}] already exists, continuing...", finalizer);

        return Ok(false);
    }

    let new_metadata = json!({
        "metadata": {
            "finalizers": [finalizer.to_string()]
        }
    });
    client.merge_patch(resource, new_metadata).await?;
    Ok(true)
}

/// Removes our finalizer from a resource object.
///
/// # Arguments
///
/// * `client` - The Client to access Kubernetes
/// * `resource` - is the resource we want to remove the finalizer from
/// * `finalizer` - this is the actual finalizer string that we want to remove
pub async fn remove_finalizer<T>(
    client: &Client,
    resource: &T,
    finalizer: &str,
) -> OperatorResult<T>
where
    T: Clone + Debug + DeserializeOwned + Meta,
    <T as Meta>::DynamicType: Default,
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
        Some(finalizers) => {
            let index = finalizers
                .iter()
                .position(|cur_finalizer| cur_finalizer == finalizer);

            if let Some(index) = index {
                // We found our finalizer which means that we now need to handle our deletion logic
                // And then remove the finalizer from the list.

                let finalizer_path = format!("/metadata/finalizers/{}", index);
                let patch = json_patch::Patch(vec![
                    PatchOperation::Test(TestOperation {
                        path: finalizer_path.clone(),
                        value: finalizer.into(),
                    }),
                    PatchOperation::Remove(RemoveOperation {
                        path: finalizer_path,
                    }),
                ]);

                client.json_patch(resource, patch).await
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
