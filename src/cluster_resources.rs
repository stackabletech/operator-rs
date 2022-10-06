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
            apps::v1::{DaemonSet, StatefulSet},
            core::v1::{ConfigMap, ObjectReference, Service},
        },
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    },
    kube::{Resource, ResourceExt},
    labels::{APP_INSTANCE_LABEL, APP_MANAGED_BY_LABEL, APP_NAME_LABEL},
};
use k8s_openapi::api::{core::v1::ServiceAccount, rbac::v1::RoleBinding};
use kube::core::ErrorResponse;
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, info};

/// A cluster resource handled by [`ClusterResources`].
///
/// This trait is used in the function signatures of [`ClusterResources`] and restricts the
/// possible kinds of resources. [`ClusterResources::delete_orphaned_resources`] iterates over all
/// implementations and removes the orphaned resources. Therefore if a new implementation is added,
/// it must be added to [`ClusterResources::delete_orphaned_resources`] as well.
pub trait ClusterResource:
    Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Serialize
{
}

impl ClusterResource for ConfigMap {}
impl ClusterResource for DaemonSet {}
impl ClusterResource for Service {}
impl ClusterResource for StatefulSet {}
impl ClusterResource for ServiceAccount {}
impl ClusterResource for RoleBinding {}

/// A structure containing the cluster resources.
///
/// Cluster resources can be added and orphaned resources are deleted. A cluster resource becomes
/// orphaned for instance if a role or role group is removed from a cluster specification.
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
///     DeleteOrphanedClusterResources {
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
///         .delete_orphaned_resources(&client)
///         .await
///         .map_err(|source| Error::DeleteOrphanedClusterResources { source })?;
///
///     Ok(Action::await_change())
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ClusterResources {
    /// The namespace of the cluster
    namespace: String,
    /// The name of the cluster
    app_instance: String,
    /// The name of the application
    app_name: String,
    /// The manager of the cluster resources, e.g. the controller
    manager: String,
    /// The unique IDs of the cluster resources
    resource_ids: HashSet<String>,
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
            resource_ids: Default::default(),
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
    ///
    /// If the patched resource does not contain a UID then an `Error::MissingObjectKey` is
    /// returned.
    pub async fn add<T: ClusterResource>(
        &mut self,
        client: &Client,
        resource: &T,
    ) -> OperatorResult<T> {
        ClusterResources::check_label(resource.labels(), APP_INSTANCE_LABEL, &self.app_instance)?;
        ClusterResources::check_label(resource.labels(), APP_MANAGED_BY_LABEL, &self.manager)?;
        ClusterResources::check_label(resource.labels(), APP_NAME_LABEL, &self.app_name)?;

        let patched_resource = client
            .apply_patch(&self.manager, resource, resource)
            .await?;

        let resource_id = patched_resource.uid().ok_or(Error::MissingObjectKey {
            key: "metadata/uid",
        })?;

        self.resource_ids.insert(resource_id);

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

    /// Finalizes the cluster creation and deletes all orphaned resources.
    ///
    /// The orphaned resources of all kinds of resources which implement the [`ClusterResource`]
    /// trait, are deleted. A resource is seen as orphaned if it is labelled as if it belongs to
    /// this cluster instance but was not added to the cluster resources before.
    ///
    /// The following resource labels are compared:
    /// * `app.kubernetes.io/instance`
    /// * `app.kubernetes.io/managed-by`
    /// * `app.kubernetes.io/name`
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    pub async fn delete_orphaned_resources(self, client: &Client) -> OperatorResult<()> {
        tokio::try_join!(
            self.delete_orphaned_resources_of_kind::<Service>(client),
            self.delete_orphaned_resources_of_kind::<StatefulSet>(client),
            self.delete_orphaned_resources_of_kind::<DaemonSet>(client),
            self.delete_orphaned_resources_of_kind::<ConfigMap>(client),
            self.delete_orphaned_resources_of_kind::<ServiceAccount>(client),
            self.delete_orphaned_resources_of_kind::<RoleBinding>(client),
        )?;

        Ok(())
    }

    /// Deletes all deployed resources of the given kind which are labelled as if they belong to
    /// this cluster instance but are not contained in the given list.
    ///
    /// If it is forbidden to list the resources of the given kind then it is assumed that the
    /// caller is not in charge of these resources, the deletion is skipped, and no error is
    /// returned.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    ///
    /// # Errors
    ///
    /// If a deployed resource does not contain a UID then an `Error::MissingObjectKey` is
    /// returned.
    async fn delete_orphaned_resources_of_kind<T: ClusterResource>(
        &self,
        client: &Client,
    ) -> OperatorResult<()> {
        match self.list_deployed_cluster_resources::<T>(client).await {
            Ok(deployed_cluster_resources) => {
                let mut orphaned_resources = Vec::new();

                for resource in deployed_cluster_resources {
                    let resource_id = resource.uid().ok_or(Error::MissingObjectKey {
                        key: "metadata/uid",
                    })?;
                    if !self.resource_ids.contains(&resource_id) {
                        orphaned_resources.push(resource);
                    }
                }

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
            Err(Error::KubeError {
                source: kube::Error::Api(ErrorResponse { code: 403, .. }),
            }) => {
                debug!(
                    "Skipping deletion of orphaned {} because the operator is not allowed to list \
                      them and is therefore probably not in charge of them.",
                    T::plural(&())
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Creates a string containing the names and if present namespaces of the given resources
    /// sorted by name and separated with commas.
    fn print_resources<T: ClusterResource>(resources: &[T]) -> String {
        let mut output = resources
            .iter()
            .map(ClusterResources::print_resource)
            .collect::<Vec<_>>();
        output.sort();
        output.join(", ")
    }

    /// Creates a string containing the name and if present namespace of the given resource.
    fn print_resource<T: ClusterResource>(resource: &T) -> String {
        if let Some(namespace) = resource.namespace() {
            format!("{name}.{namespace}", name = resource.name_any())
        } else {
            resource.name_any()
        }
    }

    /// Lists the deployed resources with instance, name, and managed-by labels equal to this
    /// cluster instance.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    async fn list_deployed_cluster_resources<T: ClusterResource>(
        &self,
        client: &Client,
    ) -> OperatorResult<Vec<T>> {
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
