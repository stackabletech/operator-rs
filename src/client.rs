use crate::error::OperatorResult;
use crate::finalizer;
use crate::podutils;

use either::Either;
use k8s_openapi::Resource;
use kube::api::{DeleteParams, ListParams, Meta, Patch, PatchParams, PostParams};
use kube::client::{Client as KubeClient, Status};
use kube::Api;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::trace;

/// This `Client` can be used to access Kubernetes.
/// It wraps an underlying [kube::client::Client] and provides some common functionality.
#[derive(Clone)]
pub struct Client {
    client: KubeClient,
    patch_params: PatchParams,
    post_params: PostParams,
    delete_params: DeleteParams,
}

impl Client {
    pub fn new(client: KubeClient, field_manager: Option<String>) -> Self {
        Client {
            client,
            post_params: PostParams {
                field_manager: field_manager.clone(),
                ..PostParams::default()
            },

            patch_params: PatchParams {
                field_manager,
                ..PatchParams::default()
            },
            delete_params: DeleteParams::default(),
        }
    }

    /// Returns a [kube::client::Client]] that can be freely used.
    /// It does not need to be cloned before first use.
    pub fn as_kube_client(&self) -> KubeClient {
        self.client.clone()
    }

    /// Retrieves a single instance of the requested resource type with the given name.
    pub async fn get<T>(&self, resource_name: &str, namespace: Option<String>) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        Ok(self.get_api(namespace).get(resource_name).await?)
    }

    /// Retrieves all instances of the requested resource type.
    /// NOTE: This _currently_ does not support label selectors
    pub async fn list<T>(&self, namespace: Option<String>) -> OperatorResult<Vec<T>>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        Ok(self
            .get_api(namespace)
            .list(&ListParams::default())
            .await?
            .items)
    }

    /// Creates a new resource.
    pub async fn create<T>(&self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Serialize,
    {
        Ok(self
            .get_api(Meta::namespace(resource))
            .create(&self.post_params, resource)
            .await?)
    }

    /// Patches a resource using the `MERGE` patch strategy.
    /// This will fail for objects that do not exist yet.
    pub async fn merge_patch<T, P>(&self, resource: &T, patch: P) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta,
        P: Serialize,
    {
        self.patch(resource, Patch::Merge(patch), &self.patch_params)
            .await
    }

    /// Patches a resource using the `APPLY` patch strategy.
    /// This will _create_ or _update_ existing resources.
    pub async fn apply_patch<T, P>(&self, resource: &T, patch: P) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta,
        P: Serialize,
    {
        self.patch(resource, Patch::Apply(patch), &self.patch_params)
            .await
    }

    async fn patch<T, P>(
        &self,
        resource: &T,
        patch: Patch<P>,
        patch_params: &PatchParams,
    ) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta,
        P: Serialize,
    {
        Ok(self
            .get_api(Meta::namespace(resource))
            .patch(&Meta::name(resource), patch_params, &patch)
            .await?)
    }

    /// Replaces a resource.
    /// This will _update_ an existing resource.
    /// NOTE: I do not know what the difference is between `update` and `apply_patch` for updates.
    pub async fn update<T>(&self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Serialize,
    {
        Ok(self
            .get_api(Meta::namespace(resource))
            .replace(&Meta::name(resource), &self.post_params, resource)
            .await?)
    }

    /// This deletes a resource _if it is not deleted already_.
    ///
    /// It checks whether the resource is already deleted by looking at the `deletion_timestamp`
    /// of the resource using the [`finalizer::has_deletion_stamp`] method.
    /// If that is the case it'll return a `Ok(None)`.
    ///    
    /// In case the object is actually deleted or marked for deletion there are two possible
    /// return types.
    /// Which of the two are returned depends on the API being called.
    /// Take a look at the Kubernetes API reference.
    /// Some `delete` endpoints return the object and others return a `Status` object.
    pub async fn delete<T>(&self, resource: &T) -> OperatorResult<Option<Either<T, Status>>>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        if finalizer::has_deletion_stamp(resource) {
            trace!(
                "Resource ([{}]) already has `deletion_timestamp`, not deleting",
                podutils::get_log_name(resource)
            );
            Ok(None)
        } else {
            trace!(
                "Resource ([{}]) does not have a `deletion_timestamp`, deleting now",
                podutils::get_log_name(resource)
            );
            let api: Api<T> = self.get_api(Meta::namespace(resource));
            Ok(Some(
                api.delete(&Meta::name(resource), &self.delete_params)
                    .await?,
            ))
        }
    }

    /// Returns an [kube::Api] object which is either namespaced or not depending on whether
    /// or not a namespace string is passed in.
    pub fn get_api<T>(&self, namespace: Option<String>) -> Api<T>
    where
        T: Meta,
    {
        match namespace {
            None => self.get_all_api(),
            Some(namespace) => self.get_namespaced_api(&namespace),
        }
    }

    pub fn get_all_api<T>(&self) -> Api<T>
    where
        T: Resource,
    {
        Api::all(self.client.clone())
    }

    pub fn get_namespaced_api<T>(&self, namespace: &str) -> Api<T>
    where
        T: Resource,
    {
        Api::namespaced(self.client.clone(), namespace)
    }
}

pub async fn create_client(field_manager: Option<String>) -> OperatorResult<Client> {
    Ok(Client::new(
        kube::Client::try_default().await?,
        field_manager,
    ))
}
