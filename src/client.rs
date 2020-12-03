use crate::error::Error;

use kube::api::{Meta, PatchParams, PostParams};
use kube::client::Client as KubeClient;
use kube::Api;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// This `Client` can be used to access Kubernetes.
/// It wraps an underlying [kube::client::Client] and provides some common functionality.
#[derive(Clone)]
pub struct Client {
    client: KubeClient,
    post_params: PostParams,
    patch_params: PatchParams,
}

impl Client {
    pub fn new(client: KubeClient) -> Self {
        Client {
            client,
            post_params: PostParams::default(),
            patch_params: PatchParams::default(),
        }
    }

    /// Returns a [kube::client::Client]] that can be freely used.
    /// It does not need to be cloned before first use.
    pub fn kube_client(&self) -> KubeClient {
        self.client.clone()
    }

    pub async fn get<T>(&self, resource_name: &str, namespace: Option<String>) -> Result<T, Error>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        self.get_api(namespace)
            .get(resource_name)
            .await
            .map_err(Error::from)
    }

    pub async fn create<T>(&self, resource: &T) -> Result<T, Error>
    where
        T: Clone + DeserializeOwned + Meta + Serialize,
    {
        self.get_api(Meta::namespace(resource))
            .create(&self.post_params, resource)
            .await
            .map_err(Error::from)
    }

    pub async fn patch<T>(&self, resource: &T, patch: Vec<u8>) -> Result<T, Error>
    where
        T: Clone + DeserializeOwned + Meta,
    {
        self.get_api(Meta::namespace(resource))
            .patch(&Meta::name(resource), &self.patch_params, patch)
            .await
            .map_err(Error::from)
    }

    pub async fn update<T>(&self, resource: &T) -> Result<T, Error>
    where
        T: Clone + DeserializeOwned + Meta + Serialize,
    {
        self.get_api(Meta::namespace(resource))
            .replace(&Meta::name(resource), &self.post_params, resource)
            .await
            .map_err(Error::from)
    }

    /// Returns an [kube::Api] object which is either namespaced or not depending on whether
    /// a resource is passed in or not and whether that has a namespace or not.
    fn get_api<T>(&self, namespace: Option<String>) -> Api<T>
    where
        T: Meta,
    {
        match namespace {
            None => self.get_all_api(),
            Some(namespace) => self.get_namespaced_api(&namespace),
        }
    }

    fn get_all_api<T>(&self) -> Api<T>
    where
        T: k8s_openapi::Resource,
    {
        Api::all(self.client.clone())
    }

    fn get_namespaced_api<T>(&self, namespace: &str) -> Api<T>
    where
        T: k8s_openapi::Resource,
    {
        Api::namespaced(self.client.clone(), namespace)
    }
}
