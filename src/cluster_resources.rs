//! A structure containing the cluster resources.

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
};

use crate::{
    client::Client,
    error::{Error, OperatorResult},
    k8s_openapi::{
        api::{
            apps::v1::StatefulSet,
            core::v1::{ConfigMap, ObjectReference, Service},
        },
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    },
    kube::{Resource, ResourceExt},
    labels::{APP_INSTANCE_LABEL, APP_MANAGED_BY_LABEL, APP_NAME_LABEL},
};
use serde::{de::DeserializeOwned, Serialize};
use tracing::info;

/// A structure containing the cluster resources.
///
/// Cluster resources can be added and on finalizing, orphaned resources are deleted. A cluster
/// resource becomes orphaned for instance if a role or role group is removed from a cluster
/// specification.
///
/// # Examples
///
/// ```
/// use k8s_openapi::api::apps::v1::StatefulSet;
/// use k8s_openapi::api::core::v1::{ConfigMap, Service};
/// use kube::CustomResource;
/// use kube::core::{Resource, CustomResourceExt};
/// use kube::runtime::controller::Action;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
/// use stackable_operator::client::Client;
/// use stackable_operator::cluster_resources::ClusterResources;
/// use stackable_operator::product_config_utils::ValidatedRoleConfigByPropertyKind;
/// use stackable_operator::role_utils::Role;
/// use std::sync::Arc;
///
/// const APP_NAME: &str = "app";
/// const FIELD_MANAGER_SCOPE: &str = "appcluster";
///
/// #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, Serialize)]
/// #[kube(
///     group = "app.stackable.tech",
///     version = "v1",
///     kind = "AppCluster",
///     plural = "AppClusters",
///     namespaced,
/// )]
/// struct AppClusterSpec {}
///
/// enum Error {
///     CreateClusterResources {
///         source: stackable_operator::error::Error,
///     },
///     AddClusterResource {
///         source: stackable_operator::error::Error,
///     },
///     FinalizeClusterResources {
///         source: stackable_operator::error::Error,
///     },
/// };
///
/// async fn reconcile(app: Arc<AppCluster>, client: Arc<Client>) -> Result<Action, Error> {
///     let validated_config = ValidatedRoleConfigByPropertyKind::default();
///
///     let mut cluster_resources = ClusterResources::new(
///         APP_NAME,
///         FIELD_MANAGER_SCOPE,
///         &app.object_ref(&()),
///     )
///     .map_err(|source| Error::CreateClusterResources { source })?;
///
///     let role_service = Service::default();
///     let patched_role_service =
///         cluster_resources.add(&client, &role_service)
///             .await
///             .map_err(|source| Error::AddClusterResource { source })?;
///
///     for (role_name, group_config) in validated_config.iter() {
///         for (rolegroup_name, rolegroup_config) in group_config.iter() {
///             let rolegroup_service = Service::default();
///             cluster_resources.add(&client, &rolegroup_service)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_configmap = ConfigMap::default();
///             cluster_resources.add(&client, &rolegroup_configmap)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_statefulset = StatefulSet::default();
///             cluster_resources.add(&client, &rolegroup_statefulset)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///         }
///     }
///
///     let discovery_configmap = ConfigMap::default();
///     let patched_discovery_configmap =
///         cluster_resources.add(&client, &discovery_configmap)
///             .await
///             .map_err(|source| Error::AddClusterResource { source })?;
///
///     cluster_resources
///         .finalize(&client)
///         .await
///         .map_err(|source| Error::FinalizeClusterResources { source })?;
///
///     Ok(Action::await_change())
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ClusterResources {
    namespace: String,
    app_instance: String,
    app_name: String,
    manager: String,
    resources: HashSet<ResourceId>,
}

impl ClusterResources {
    /// Constructs new `ClusterResources`.
    ///
    /// # Arguments
    ///
    /// * `app_name` - The lower-case application name used in the resource labels, e.g.
    ///   "zookeeper"
    /// * `manager` - The manager of these cluster resources, e.g.
    ///   "zookeeper-operator_zk-controller". The added resources must contain the content of this
    ///   field in the `app.kubernetes.io/managed-by` label. It must be different for each
    ///   controller in the operator, otherwise resources created by another controller are detected
    ///   as orphaned in this cluster and are deleted. This value is also used for the field manager
    ///   scope when applying resources.
    /// * `cluster` - A reference to the cluster containing the name and namespace of the cluster
    ///
    /// # Errors
    ///
    /// If `cluster` does not contain a namespace and a name then an `Error::MissingObjectKey` is
    /// returned.
    pub fn new(app_name: &str, manager: &str, cluster: &ObjectReference) -> OperatorResult<Self> {
        let namespace = cluster
            .namespace
            .to_owned()
            .ok_or(Error::MissingObjectKey { key: "namespace" })?;
        let app_instance = cluster
            .name
            .to_owned()
            .ok_or(Error::MissingObjectKey { key: "name" })?;

        Ok(ClusterResources {
            namespace,
            app_instance,
            app_name: app_name.into(),
            manager: manager.into(),
            resources: Default::default(),
        })
    }

    /// Adds a resource to the cluster resources.
    ///
    /// The resource will be patched and the patched resource will be returned.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `resource` - A resource to add to the cluster
    ///
    /// # Errors
    ///
    /// If the labels of the given resource are not set properly then an `Error::MissingLabel` or
    /// `Error::UnexpectedLabelContent` is returned. The expected labels are:
    /// * `app.kubernetes.io/instance = <cluster.name>`
    /// * `app.kubernetes.io/managed-by = <app_name>-operator`
    /// * `app.kubernetes.io/name = <app_name>`
    pub async fn add<T>(&mut self, client: &Client, resource: &T) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Serialize,
    {
        ClusterResources::check_label(resource.labels(), APP_INSTANCE_LABEL, &self.app_instance)?;
        ClusterResources::check_label(resource.labels(), APP_MANAGED_BY_LABEL, &self.manager)?;
        ClusterResources::check_label(resource.labels(), APP_NAME_LABEL, &self.app_name)?;

        let patched_resource = client
            .apply_patch(&self.manager, resource, resource)
            .await?;

        self.resources.insert((&patched_resource).into());

        Ok(patched_resource)
    }

    /// Checks that the given `labels` contain the given `label` with the given `expected_content`.
    ///
    /// # Arguments
    ///
    /// * `labels` - The labels to check
    /// * `label` - The expected label
    /// * `expected_content` - The expected content of the label
    ///
    /// # Errors
    ///
    /// If `labels` does not contain `label` then an `Error::MissingLabel` is returned.
    ///
    /// If `labels` contains the given `label` but not with the `expected_content` then an
    /// `Error::UnexpectedLabelContent` is returned
    fn check_label(
        labels: &BTreeMap<String, String>,
        label: &'static str,
        expected_content: &str,
    ) -> OperatorResult<()> {
        if let Some(actual_content) = labels.get(label) {
            if expected_content == actual_content {
                Ok(())
            } else {
                Err(Error::UnexpectedLabelContent {
                    label,
                    expected_content: expected_content.into(),
                    actual_content: actual_content.into(),
                })
            }
        } else {
            Err(Error::MissingLabel { label })
        }
    }

    /// Finalizes the cluster creation.
    ///
    /// All orphaned resources, i.e. resources which are labelled as if they belong to this cluster
    /// instance but were not added to the cluster resources, are deleted.
    ///
    /// The following resource types are considered:
    /// * `ConfigMap`
    /// * `Service`
    /// * `StatefulSet`
    ///
    /// The following resource labels are compared:
    /// * `app.kubernetes.io/instance`
    /// * `app.kubernetes.io/managed-by`
    /// * `app.kubernetes.io/name`
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    pub async fn finalize(self, client: &Client) -> OperatorResult<()> {
        self.delete_orphaned_resources::<Service>(client).await?;
        self.delete_orphaned_resources::<StatefulSet>(client)
            .await?;
        self.delete_orphaned_resources::<ConfigMap>(client).await?;

        Ok(())
    }

    /// Deletes all deployed resources which are labelled as if they belong to this cluster
    /// instance but are not contained in the given list.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    async fn delete_orphaned_resources<T>(&self, client: &Client) -> OperatorResult<()>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
    {
        let deployed_cluster_resources = self.list_deployed_cluster_resources::<T>(client).await?;

        let orphaned_resources = deployed_cluster_resources
            .into_iter()
            .filter(|r| !self.resources.contains(&r.into()))
            .collect::<Vec<_>>();

        if !orphaned_resources.is_empty() {
            info!(
                "Deleting orphaned {}: {}",
                T::plural(&()),
                ClusterResources::print_resources(&orphaned_resources),
            );
            for resource in orphaned_resources.iter() {
                client.delete(resource).await?;
            }
        }

        Ok(())
    }

    /// Creates a string containing the names and if present namespaces of the given resources
    /// sorted by name and separated with commas.
    fn print_resources<T>(resources: &[T]) -> String
    where
        T: Resource<DynamicType = ()>,
    {
        let mut output = resources
            .iter()
            .map(ClusterResources::print_resource)
            .collect::<Vec<_>>();
        output.sort();
        output.join(", ")
    }

    /// Creates a string containing the name and if present namespace of the given resource.
    fn print_resource<T>(resource: &T) -> String
    where
        T: Resource<DynamicType = ()>,
    {
        if let Some(namespace) = resource.namespace() {
            format!("{name}.{namespace}", name = resource.name())
        } else {
            resource.name()
        }
    }

    /// Lists the deployed resources with instance, name, and managed-by labels equal to this
    /// cluster instance.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    async fn list_deployed_cluster_resources<T>(&self, client: &Client) -> OperatorResult<Vec<T>>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
    {
        let label_selector = LabelSelector {
            match_expressions: Some(vec![
                LabelSelectorRequirement {
                    key: APP_INSTANCE_LABEL.into(),
                    operator: "In".into(),
                    values: Some(vec![self.app_instance.to_owned()]),
                },
                LabelSelectorRequirement {
                    key: APP_NAME_LABEL.into(),
                    operator: "In".into(),
                    values: Some(vec![self.app_name.to_owned()]),
                },
                LabelSelectorRequirement {
                    key: APP_MANAGED_BY_LABEL.into(),
                    operator: "In".into(),
                    values: Some(vec![self.manager.to_owned()]),
                },
            ]),
            ..Default::default()
        };

        let resources = client
            .list_with_label_selector::<T>(Some(&self.namespace), &label_selector)
            .await?;

        Ok(resources)
    }
}

/// A resource ID consisting of kind, optional namespace, and name.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ResourceId {
    kind: String,
    namespace: Option<String>,
    name: String,
}

impl<T> From<&T> for ResourceId
where
    T: Resource<DynamicType = ()>,
{
    fn from(resource: &T) -> Self {
        Self {
            kind: T::kind(&()).into(),
            namespace: resource.namespace(),
            name: resource.name(),
        }
    }
}
