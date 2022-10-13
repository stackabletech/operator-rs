use crate::error::{Error, OperatorResult};
use crate::label_selector;

use either::Either;
use futures::StreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams, PostParams, Resource, ResourceExt};
use kube::client::Client as KubeClient;
use kube::core::Status;
use kube::runtime::wait::delete::delete_and_finalize;
use kube::runtime::WatchStreamExt;
use kube::{Api, Config};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::convert::TryFrom;
use std::fmt::{Debug, Display};
use tracing::trace;

/// This `Client` can be used to access Kubernetes.
/// It wraps an underlying [kube::client::Client] and provides some common functionality.
#[derive(Clone)]
pub struct Client {
    client: KubeClient,
    patch_params: PatchParams,
    post_params: PostParams,
    delete_params: DeleteParams,
    /// Default namespace as defined in the kubeconfig this client has been created from.
    pub default_namespace: String,
}

impl Client {
    pub fn new(
        client: KubeClient,
        field_manager: Option<String>,
        default_namespace: String,
    ) -> Self {
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
            default_namespace,
        }
    }

    /// Server-side apply requires a `field_manager` that uniquely identifies a single usage site,
    /// since it will revert changes that are owned by the `field_manager` but not part of the Apply request.
    fn apply_patch_params(&self, field_manager_scope: impl Display) -> PatchParams {
        let mut params = self.patch_params.clone();
        // According to https://kubernetes.io/docs/reference/using-api/server-side-apply/#using-server-side-apply-in-a-controller we should always force conflicts in controllers.
        params.force = true;
        if let Some(manager) = &mut params.field_manager {
            *manager = format!("{}/{}", manager, field_manager_scope);
        }
        params
    }

    /// Returns a [kube::client::Client]] that can be freely used.
    /// It does not need to be cloned before first use.
    pub fn as_kube_client(&self) -> KubeClient {
        self.client.clone()
    }

    /// Retrieves a single instance of the requested resource type with the given name.
    pub async fn get<T>(&self, resource_name: &str, namespace: &T::Namespace) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        Ok(self.get_api(namespace).get(resource_name).await?)
    }

    /// Retrieves a single instance of the requested resource type with the given name, if it exists.
    pub async fn get_opt<T>(
        &self,
        resource_name: &str,
        namespace: &T::Namespace,
    ) -> OperatorResult<Option<T>>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        Ok(self.get_api(namespace).get_opt(resource_name).await?)
    }

    /// Returns Ok(true) if the resource has been registered in Kubernetes, Ok(false) if it could
    /// not be found and Error in any other case (e.g. connection to Kubernetes failed in some way).
    /// Kubernetes does not offer a pure exists check. Therefore we currently use the get() method
    /// and ignore the (in case of existing) returned resource. We should replace this with a pure
    /// exists method as soon as it becomes available (e.g. only returning Ok/Success) to reduce
    /// network traffic.
    #[deprecated(since = "0.24.0", note = "Replaced by `get_opt`")]
    pub async fn exists<T>(
        &self,
        resource_name: &str,
        namespace: &T::Namespace,
    ) -> OperatorResult<bool>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        self.get_opt::<T>(resource_name, namespace)
            .await
            .map(|obj| obj.is_some())
    }

    /// Retrieves all instances of the requested resource type.
    ///
    /// The `list_params` parameter can be used to pass in a `label_selector` or a `field_selector`.
    pub async fn list<T>(
        &self,
        namespace: &T::Namespace,
        list_params: &ListParams,
    ) -> OperatorResult<Vec<T>>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        Ok(self.get_api(namespace).list(list_params).await?.items)
    }

    /// Lists resources from the API using a LabelSelector.
    ///
    /// This takes a LabelSelector and converts it into a query string using [`label_selector::convert_label_selector_to_query_string`].
    ///
    /// # Arguments
    ///
    /// - `namespace` - Optional name of the namespace to search in. Otherwise searches in all namespaces.
    /// - `selector` - A reference to a `LabelSelector` to filter out pods
    pub async fn list_with_label_selector<T>(
        &self,
        namespace: &T::Namespace,
        selector: &LabelSelector,
    ) -> OperatorResult<Vec<T>>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        let selector_string = label_selector::convert_label_selector_to_query_string(selector)?;
        trace!("Listing for LabelSelector [{}]", selector_string);
        let list_params = ListParams {
            label_selector: Some(selector_string),
            ..ListParams::default()
        };
        self.list(namespace, &list_params).await
    }

    /// Creates a new resource.
    pub async fn create<T>(&self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + Serialize + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        Ok(self
            .get_api(resource.get_namespace())
            .create(&self.post_params, resource)
            .await?)
    }

    /// Patches a resource using the `MERGE` patch strategy described
    /// in [JSON Merge Patch](https://tools.ietf.org/html/rfc7386)
    /// This will fail for objects that do not exist yet.
    pub async fn merge_patch<T, P>(&self, resource: &T, patch: P) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
        P: Debug + Serialize,
    {
        self.patch(resource, Patch::Merge(patch), &self.patch_params)
            .await
    }

    /// Patches a resource using the `APPLY` patch strategy.
    /// This is a [_Server-Side Apply_](https://kubernetes.io/docs/reference/using-api/server-side-apply/)
    /// and the merge strategy can differ from field to field and will be defined by the
    /// schema of the resource in question.
    /// This will _create_ or _update_ existing resources.
    pub async fn apply_patch<T, P>(
        &self,
        field_manager_scope: &str,
        resource: &T,
        patch: P,
    ) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
        P: Debug + Serialize,
    {
        self.patch(
            resource,
            Patch::Apply(patch),
            &self.apply_patch_params(field_manager_scope),
        )
        .await
    }

    /// Patches a resource using the `JSON` patch strategy described in [JavaScript Object Notation (JSON) Patch](https://tools.ietf.org/html/rfc6902).
    pub async fn json_patch<T>(&self, resource: &T, patch: json_patch::Patch) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        // The `()` type is not used. I need to provide _some_ type just to get it to compile.
        // But the type is not used _at all_ for the `Json` variant so I'd argue it's okay to
        // provide any type here.
        // This is definitely a hack though but there is currently no better way.
        // See also: https://github.com/clux/kube-rs/pull/456
        let patch = Patch::Json::<()>(patch);
        self.patch(resource, patch, &self.patch_params).await
    }

    async fn patch<T, P>(
        &self,
        resource: &T,
        patch: Patch<P>,
        patch_params: &PatchParams,
    ) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
        P: Debug + Serialize,
    {
        Ok(self
            .get_api(resource.get_namespace())
            .patch(&resource.name_any(), patch_params, &patch)
            .await?)
    }

    /// Patches subresource status in a given Resource using apply strategy.
    /// The subresource status must be defined beforehand in the Crd.
    pub async fn apply_patch_status<T, S>(
        &self,
        field_manager_scope: &str,
        resource: &T,
        status: &S,
    ) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + GetApi,
        <T as Resource>::DynamicType: Default,
        S: Debug + Serialize,
    {
        let meta = resource.meta();
        let new_status = Patch::Apply(serde_json::json!({
            "apiVersion": T::api_version(&()),
            "kind": T::kind(&()),
            "metadata": {
                "name": meta.name,
                "namespace": meta.namespace,
            },
            "status": status
        }));

        self.patch_status(
            resource,
            new_status,
            &self.apply_patch_params(field_manager_scope),
        )
        .await
    }

    /// Patches subresource status in a given Resource using merge strategy.
    /// The subresource status must be defined beforehand in the Crd.
    pub async fn merge_patch_status<T, S>(&self, resource: &T, status: &S) -> OperatorResult<T>
    where
        T: DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
        S: Debug + Serialize,
    {
        let new_status = Patch::Merge(serde_json::json!({ "status": status }));

        self.patch_status(resource, new_status, &self.patch_params)
            .await
    }

    /// Patches subresource status in a given Resource using merge strategy.
    /// The subresource status must be defined beforehand in the Crd.
    /// Patches a resource using the `JSON` patch strategy described in [JavaScript Object Notation (JSON) Patch](https://tools.ietf.org/html/rfc6902).
    pub async fn json_patch_status<T>(
        &self,
        resource: &T,
        patch: json_patch::Patch,
    ) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        // The `()` type is not used. I need to provide _some_ type just to get it to compile.
        // But the type is not used _at all_ for the `Json` variant so I'd argue it's okay to
        // provide any type here.
        // This is definitely a hack though but there is currently no better way.
        // See also: https://github.com/clux/kube-rs/pull/456
        let patch = Patch::Json::<()>(patch);
        self.patch_status(resource, patch, &self.patch_params).await
    }

    /// There are four different patch strategies:
    /// 1) Apply (<https://kubernetes.io/docs/reference/using-api/api-concepts/#server-side-apply>)
    ///   Starting from Kubernetes v1.18, you can enable the Server Side Apply feature so that the control plane tracks managed fields for all newly created objects.
    /// 2) Json (<https://tools.ietf.org/html/rfc6902>):
    ///   This is supported on crate feature jsonpatch only
    /// 3) Merge (<https://tools.ietf.org/html/rfc7386>):
    ///   For example, if you want to update a list you have to specify the complete list and update everything
    /// 4) Strategic (not for CustomResource)
    ///   With a strategic merge patch, a list is either replaced or merged depending on its patch strategy.
    ///   The patch strategy is specified by the value of the patchStrategy key in a field tag in the Kubernetes source code.
    ///   For example, the Containers field of PodSpec struct has a patchStrategy of merge.
    async fn patch_status<T, S>(
        &self,
        resource: &T,
        patch: Patch<S>,
        patch_params: &PatchParams,
    ) -> OperatorResult<T>
    where
        T: DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
        S: Debug + Serialize,
    {
        let api = self.get_api(resource.get_namespace());
        Ok(api
            .patch_status(&resource.name_any(), patch_params, &patch)
            .await?)
    }

    /// This will _update_ an existing resource.
    /// The operation is called `replace` in the Kubernetes API.
    /// While a `patch` can just update a partial object
    /// a `update` will always replace the full object.
    pub async fn update<T>(&self, resource: &T) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource + Serialize + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        Ok(self
            .get_api(resource.get_namespace())
            .replace(&resource.name_any(), &self.post_params, resource)
            .await?)
    }

    /// This deletes a resource _if it is not deleted already_.
    ///
    /// In case the object is actually deleted or marked for deletion there are two possible
    /// return types.
    /// Which of the two are returned depends on the API being called.
    /// Take a look at the Kubernetes API reference.
    /// Some `delete` endpoints return the object and others return a `Status` object.
    pub async fn delete<T>(&self, resource: &T) -> OperatorResult<Either<T, Status>>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        let api: Api<T> = self.get_api(resource.get_namespace());
        Ok(api
            .delete(&resource.name_any(), &self.delete_params)
            .await?)
    }

    /// This deletes a resource _if it is not deleted already_ and waits until the deletion is
    /// performed by Kubernetes.
    ///
    /// It calls `delete` to perform the deletion.
    ///
    /// Afterwards it loops and checks regularly whether the resource has been deleted
    /// from Kubernetes
    pub async fn ensure_deleted<T>(&self, resource: T) -> OperatorResult<()>
    where
        T: Clone + Debug + DeserializeOwned + Resource + GetApi + Send + 'static,
        <T as Resource>::DynamicType: Default,
    {
        Ok(delete_and_finalize(
            self.get_api::<T>(resource.get_namespace()),
            resource
                .meta()
                .name
                .as_deref()
                .ok_or(Error::MissingObjectKey {
                    key: "metadata.name",
                })?,
            &self.delete_params,
        )
        .await?)
    }

    /// Returns an [kube::Api] object which is either namespaced or not depending on whether
    /// or not a namespace string is passed in.
    pub fn get_api<T>(&self, namespace: &T::Namespace) -> Api<T>
    where
        T: Resource + GetApi,
        <T as Resource>::DynamicType: Default,
    {
        T::get_api(self.client.clone(), namespace)
    }

    pub fn get_all_api<T>(&self) -> Api<T>
    where
        T: Resource,
        <T as Resource>::DynamicType: Default,
    {
        Api::all(self.client.clone())
    }

    #[deprecated(note = "Use Api::get_api instead", since = "0.26.0")]
    pub fn get_namespaced_api<T>(&self, namespace: &str) -> Api<T>
    where
        T: Resource<Scope = NamespaceResourceScope>,
        <T as Resource>::DynamicType: Default,
    {
        self.get_api(namespace)
    }

    /// Waits indefinitely until resources matching given `ListParams` are created in Kubernetes.
    /// If the resource is already present, this method just returns. Makes no assumptions about resource's state,
    /// e.g. a pod created could be created, but not in a ready state.
    ///
    /// # Arguments
    ///
    /// - `namespace` - Optional namespace to look for the resources in.
    /// - `lp` - Parameters to filter resources to wait for in given namespace.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kube::api::ListParams;
    /// use std::time::Duration;
    /// use tokio::time::error::Elapsed;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use stackable_operator::client::{Client, create_client};
    ///
    /// #[tokio::main]
    /// async fn main(){
    /// let client: Client = create_client(None).await.expect("Unable to construct client.");
    /// let lp: ListParams =
    ///         ListParams::default().fields(&format!("metadata.name=nonexistent-pod"));
    ///
    /// // Will time out in 1 second unless the nonexistent-pod actually exists
    ///  let wait_created_result: Result<(), Elapsed> = tokio::time::timeout(
    ///          Duration::from_secs(1),
    ///          client.wait_created::<Pod>(Some(&client.default_namespace), lp.clone()),
    ///      )
    ///      .await;
    /// }
    /// ```
    ///
    pub async fn wait_created<T>(&self, namespace: &T::Namespace, lp: ListParams)
    where
        T: Resource + GetApi + Clone + Debug + DeserializeOwned + Send + 'static,
        <T as Resource>::DynamicType: Default,
    {
        let api: Api<T> = self.get_api(namespace);
        let watcher = kube::runtime::watcher(api, lp).boxed();
        watcher
            .applied_objects()
            .skip_while(|res| std::future::ready(res.is_err()))
            .next()
            .await;
    }
}

/// Helper trait for getting [`kube::Api`] instances for a Kubernetes resource's scope
///
/// Not intended to be implemented manually, it is blanket-implemented for all types that implement [`Resource`]
/// for either the [namespace](`NamespaceResourceScope`) or [cluster](`ClusterResourceScope`) scopes.
pub trait GetApi: Resource + Sized {
    /// The namespace type for `Self`'s scope.
    ///
    /// This will be [`str`] for namespaced resource, and [`()`] for cluster-scoped resources.
    type Namespace: ?Sized;
    /// Get a [`kube::Api`] for `Self`'s native scope..
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self>
    where
        Self::DynamicType: Default;
    /// Get the namespace of `Self`.
    fn get_namespace(&self) -> &Self::Namespace;
    /// Coerce a string namespace into `Self::Namespace`.
    fn namespace_from_str(ns: &str) -> &Self::Namespace;
}

impl<K> GetApi for K
where
    K: Resource,
    (K, K::Scope): GetApiImpl<Resource = K>,
{
    type Namespace = <(K, K::Scope) as GetApiImpl>::Namespace;
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self>
    where
        Self::DynamicType: Default,
    {
        <(K, K::Scope) as GetApiImpl>::get_api(client, ns)
    }
    fn get_namespace(&self) -> &Self::Namespace {
        <(K, K::Scope) as GetApiImpl>::get_namespace(self)
    }
    fn namespace_from_str(ns: &str) -> &Self::Namespace {
        <(K, K::Scope) as GetApiImpl>::namespace_from_str(ns)
    }
}

#[doc(hidden)]
// Workaround for https://github.com/rust-lang/rust/issues/20400
pub trait GetApiImpl {
    type Resource: Resource;
    type Namespace: ?Sized;
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self::Resource>
    where
        <Self::Resource as Resource>::DynamicType: Default;
    fn get_namespace(res: &Self::Resource) -> &Self::Namespace;
    fn namespace_from_str(ns: &str) -> &Self::Namespace;
}

impl<K> GetApiImpl for (K, NamespaceResourceScope)
where
    K: Resource<Scope = NamespaceResourceScope>,
{
    type Resource = K;
    type Namespace = str;
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<K>
    where
        <Self::Resource as Resource>::DynamicType: Default,
    {
        Api::namespaced(client, ns)
    }
    fn get_namespace(res: &Self::Resource) -> &Self::Namespace {
        res.meta().namespace.as_deref().unwrap_or_default()
    }
    fn namespace_from_str(ns: &str) -> &Self::Namespace {
        ns
    }
}

impl<K> GetApiImpl for (K, ClusterResourceScope)
where
    K: Resource<Scope = ClusterResourceScope>,
{
    type Resource = K;
    type Namespace = ();
    fn get_api(client: kube::Client, (): &Self::Namespace) -> kube::Api<K>
    where
        <Self::Resource as Resource>::DynamicType: Default,
    {
        Api::all(client)
    }
    fn get_namespace(_res: &Self::Resource) -> &Self::Namespace {
        &()
    }
    fn namespace_from_str(_ns: &str) -> &Self::Namespace {
        &()
    }
}

pub async fn create_client(field_manager: Option<String>) -> OperatorResult<Client> {
    let kubeconfig: Config = kube::Config::infer()
        .await
        .map_err(kube::Error::InferConfig)?;
    let default_namespace = kubeconfig.default_namespace.clone();
    Ok(Client::new(
        kube::Client::try_from(kubeconfig)?,
        field_manager,
        default_namespace,
    ))
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use k8s_openapi::api::core::v1::{Container, Pod, PodSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
    use kube::api::{ListParams, ObjectMeta, PostParams, ResourceExt};
    use kube::runtime::watcher::Event;
    use std::collections::BTreeMap;
    use std::time::Duration;
    use tokio::time::error::Elapsed;

    #[tokio::test]
    #[ignore = "Tests depending on Kubernetes are not ran by default"]
    async fn k8s_test_wait_created() {
        let client = super::create_client(None)
            .await
            .expect("KUBECONFIG variable must be configured.");

        // Definition of the pod the `wait_created` function will be waiting for.
        let pod_to_wait_for: Pod = Pod {
            metadata: ObjectMeta {
                name: Some("test-wait-created-busybox".to_owned()),
                ..ObjectMeta::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "test-wait-created-busybox".to_owned(),
                    image: Some("busybox:latest".to_owned()),
                    image_pull_policy: Some("IfNotPresent".to_owned()),
                    command: Some(vec!["sleep".into(), "infinity".into()]),
                    ..Container::default()
                }],
                termination_grace_period_seconds: Some(1),
                ..PodSpec::default()
            }),
            ..Pod::default()
        };
        let api = client.get_api::<Pod>(&client.default_namespace);
        let created_pod = api
            .create(&PostParams::default(), &pod_to_wait_for)
            .await
            .expect("Test pod not created.");
        let lp: ListParams = ListParams::default().fields(&format!(
            "metadata.name={}",
            created_pod
                .metadata
                .name
                .as_ref()
                .expect("Expected busybox pod to have metadata")
        ));
        // First, let the tested `wait_creation` function wait until the resource is present.
        // Timeout is not acceptable
        tokio::time::timeout(
            Duration::from_secs(30), // Busybox is ~5MB and sub 1 sec to start.
            client.wait_created::<Pod>(&client.default_namespace, lp.clone()),
        )
        .await
        .expect("The tested wait_created function timed out.");

        // A second, manually constructed watcher is used to verify the ListParams filter out the correct resource
        // and the `wait_created` function returned when the correct resources had been detected.
        let mut ready_watcher = kube::runtime::watcher::<Pod>(api, lp).boxed();
        while let Some(result) = ready_watcher.next().await {
            match result {
                Ok(event) => match event {
                    Event::Applied(pod) => {
                        assert_eq!("test-wait-created-busybox", pod.name_any());
                    }
                    Event::Restarted(pods) => {
                        assert_eq!(1, pods.len());
                        assert_eq!("test-wait-created-busybox", &pods[0].name_any());
                        break;
                    }
                    Event::Deleted(_) => {
                        panic!("Not expected the test_wait_created busybox pod to be deleted");
                    }
                },
                Err(_) => {
                    panic!("Error while waiting for readiness.");
                }
            }
        }

        client
            .delete(&created_pod)
            .await
            .expect("Expected test_wait_created pod to be deleted.");
    }

    #[tokio::test]
    #[ignore = "Tests depending on Kubernetes are not ran by default"]
    async fn k8s_test_wait_created_timeout() {
        let client = super::create_client(None)
            .await
            .expect("KUBECONFIG variable must be configured.");

        let lp: ListParams = ListParams::default().fields("metadata.name=nonexistent-pod");

        // There is no such pod, therefore the `wait_created` function call times out.
        let wait_created_result: Result<(), Elapsed> = tokio::time::timeout(
            Duration::from_secs(1),
            client.wait_created::<Pod>(&client.default_namespace, lp.clone()),
        )
        .await;

        assert!(wait_created_result.is_err());
    }

    #[tokio::test]
    #[ignore = "Tests depending on Kubernetes are not ran by default"]
    async fn k8s_test_list_with_label_selector() {
        let client = super::create_client(None)
            .await
            .expect("KUBECONFIG variable must be configured.");

        let mut match_labels: BTreeMap<String, String> = BTreeMap::new();
        match_labels.insert("app".to_owned(), "busybox".to_owned());
        let label_selector: LabelSelector = LabelSelector {
            match_labels: Some(match_labels.clone()),
            ..LabelSelector::default()
        };
        let no_pods: Vec<Pod> = client
            .list_with_label_selector::<Pod>(&client.default_namespace, &label_selector)
            .await
            .expect("Expected LabelSelector to return a result with zero pods.");
        assert!(no_pods.is_empty());

        let pod_to_wait_for: Pod = Pod {
            metadata: ObjectMeta {
                name: Some("pod-to-be-listed".to_owned()),
                labels: Some(match_labels.clone()),
                ..ObjectMeta::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "test-wait-created-busybox".to_owned(),
                    image: Some("busybox:latest".to_owned()),
                    image_pull_policy: Some("IfNotPresent".to_owned()),
                    command: Some(vec!["sleep".into(), "infinity".into()]),
                    ..Container::default()
                }],
                termination_grace_period_seconds: Some(1),
                ..PodSpec::default()
            }),
            ..Pod::default()
        };
        let api = client.get_api::<Pod>(&client.default_namespace);
        let created_pod = api
            .create(&PostParams::default(), &pod_to_wait_for)
            .await
            .expect("Test pod not created.");

        let one_pod: Vec<Pod> = client
            .list_with_label_selector::<Pod>(&client.default_namespace, &label_selector)
            .await
            .expect("Expected LabelSelector to return a result with zero pods.");

        assert_eq!(1, one_pod.len());
        client
            .delete(&created_pod)
            .await
            .expect("Expected Pod to be deleted");
    }
}
