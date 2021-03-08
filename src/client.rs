use crate::error::OperatorResult;
use crate::label_selector;

use either::Either;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector};
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

            // TODO: According to https://kubernetes.io/docs/reference/using-api/server-side-apply/#using-server-side-apply-in-a-controller we should always force conflicts in controllers.
            patch_params: PatchParams {
                field_manager,
                //force: true,
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
    ///
    /// The `list_params` parameter can be used to pass in a `label_selector` or a `field_selector`.
    pub async fn list<T>(
        &self,
        namespace: Option<String>,
        list_params: &ListParams,
    ) -> OperatorResult<Vec<T>>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        Ok(self.get_api(namespace).list(&list_params).await?.items)
    }

    /// Lists resources from the API using a LabelSelector.
    ///
    /// This takes a LabelSelector and converts it into a query string using [`label_selector::convert_label_selector_to_query_string`].
    pub async fn list_with_label_selector<T>(
        &self,
        selector: &LabelSelector,
    ) -> OperatorResult<Vec<T>>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        let selector_string = label_selector::convert_label_selector_to_query_string(selector)?;
        trace!("Listing for LabelSelector [{}]", selector_string);
        let list_params = ListParams {
            label_selector: Some(selector_string),
            ..ListParams::default()
        };
        self.list(None, &list_params).await
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

    /// Patches a resource using the `MERGE` patch strategy described
    /// in [JSON Merge Patch](https://tools.ietf.org/html/rfc7386)
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
    /// This is a [_Server-Side Apply_](https://kubernetes.io/docs/reference/using-api/server-side-apply/)
    /// and the merge strategy can differ from field to field and will be defined by the
    /// schema of the resource in question.
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

    /// Patches subresource status in a given Resource using apply strategy.
    /// The subresource status must be defined beforehand in the Crd.
    pub async fn apply_patch_status<T, S>(&self, resource: &T, status: &S) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Resource,
        S: Serialize,
    {
        let new_status = Patch::Apply(serde_json::json!({
            "apiVersion": T::API_VERSION,
            "kind": T::KIND,
            "status": status
        }));

        Ok(self
            .patch_status(resource, new_status, &self.patch_params)
            .await?)
    }

    /// Patches subresource status in a given Resource using merge strategy.
    /// The subresource status must be defined beforehand in the Crd.
    pub async fn merge_patch_status<T, S>(&self, resource: &T, status: &S) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Resource,
        S: Serialize,
    {
        let new_status = Patch::Merge(serde_json::json!({ "status": status }));

        Ok(self
            .patch_status(resource, new_status, &self.patch_params)
            .await?)
    }

    async fn patch_status<T, S>(
        &self,
        resource: &T,
        patch: Patch<S>,
        patch_params: &PatchParams,
    ) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta,
        S: Serialize,
    {
        // There are four different strategies:
        // 1) Apply (https://kubernetes.io/docs/reference/using-api/api-concepts/#server-side-apply)
        //   Starting from Kubernetes v1.18, you can enable the Server Side Apply feature so that the control plane tracks managed fields for all newly created objects.
        // 2) Json (https://tools.ietf.org/html/rfc6902):
        //   This is supported on crate feature jsonpatch only
        // 3) Merge (https://tools.ietf.org/html/rfc7386):
        //   For example, if you want to update a list you have to specify the complete list and update everything
        // 4) Strategic (not for CustomResource)
        //   With a strategic merge patch, a list is either replaced or merged depending on its patch strategy.
        //   The patch strategy is specified by the value of the patchStrategy key in a field tag in the Kubernetes source code.
        //   For example, the Containers field of PodSpec struct has a patchStrategy of merge.
        //

        let api = self.get_api(Meta::namespace(resource));
        Ok(api
            .patch_status(&Meta::name(resource), patch_params, &patch)
            .await?)
    }

    /// This will _update_ an existing resource.
    /// The operation is called `replace` in the Kubernetes API.
    /// While a `patch` can just update a partial object
    /// a `update` will always replace the full object.
    pub async fn update<T>(&self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Serialize,
    {
        Ok(self
            .get_api(Meta::namespace(resource))
            .replace(&Meta::name(resource), &self.post_params, resource)
            .await?)
    }

    /// Which of the two results this returns depends on the API.
    /// Take a look at the Kubernetes API reference.
    /// Some `delete` endpoints return the object and others return a `Status` object.
    pub async fn delete<T>(&self, resource: &T) -> OperatorResult<Either<T, Status>>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        let api: Api<T> = self.get_api(Meta::namespace(resource));
        Ok(api
            .delete(&Meta::name(resource), &self.delete_params)
            .await?)
    }

    /// Sets a condition on a status.
    /// This will only work if there is a `status` subresource **and** it has a `conditions` array.
    pub async fn set_condition<T>(&self, resource: &T, condition: Condition) -> OperatorResult<T>
    where
        T: Clone + DeserializeOwned + Meta + Resource,
    {
        let new_status = Patch::Apply(serde_json::json!({
            "apiVersion": T::API_VERSION,
            "kind": T::KIND,
            "status": {
                "conditions": vec![condition]
            }
        }));

        Ok(self
            .patch_status(resource, new_status, &self.patch_params)
            .await?)
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
