//! A structure containing the cluster resources.

use std::{
    collections::{hash_map::Values, BTreeMap, HashMap},
    fmt::{self, Debug, Display, Formatter},
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
///         cluster_resources.add_service(&client, &role_service)
///             .await
///             .map_err(|source| Error::AddClusterResource { source })?;
///
///     for (role_name, group_config) in validated_config.iter() {
///         for (rolegroup_name, rolegroup_config) in group_config.iter() {
///             let rolegroup_service = Service::default();
///             cluster_resources.add_service(&client, &rolegroup_service)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_configmap = ConfigMap::default();
///             cluster_resources.add_configmap(&client, &rolegroup_configmap)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_statefulset = StatefulSet::default();
///             cluster_resources.add_statefulset(&client, &rolegroup_statefulset)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///         }
///     }
///
///     let discovery_configmap = ConfigMap::default();
///     let patched_discovery_configmap =
///         cluster_resources.add_configmap(&client, &discovery_configmap)
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
    services: ResourceSet<Service>,
    configmaps: ResourceSet<ConfigMap>,
    statefulsets: ResourceSet<StatefulSet>,
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
            services: Default::default(),
            configmaps: Default::default(),
            statefulsets: Default::default(),
        })
    }

    /// Adds a service to the cluster resources.
    ///
    /// The service will be patched and the patched resource will be returned.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `service` - A service to add to the cluster
    ///
    /// # Errors
    ///
    /// If the labels of the given service are not set properly then an `Error::MissingLabel` or
    /// `Error::UnexpectedLabelContent` is returned. The expected labels are:
    /// * `app.kubernetes.io/instance = <cluster.name>`
    /// * `app.kubernetes.io/managed-by = <app_name>-operator`
    /// * `app.kubernetes.io/name = <app_name>`
    pub async fn add_service(
        &mut self,
        client: &Client,
        service: &Service,
    ) -> OperatorResult<Service> {
        self.check_labels(service.labels())?;

        let patched_service = self.patch_resource(client, service).await?;
        self.services.insert(&patched_service);

        Ok(patched_service)
    }

    /// Adds a config map to the cluster resources.
    ///
    /// The config map will be patched and the patched resource will be returned.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `configmap` - A config map to add to the cluster
    ///
    /// # Errors
    ///
    /// If the labels of the given config map are not set properly then an `Error::MissingLabel` or
    /// `Error::UnexpectedLabelContent` is returned. The expected labels are:
    /// * `app.kubernetes.io/instance = <cluster.name>`
    /// * `app.kubernetes.io/managed-by = <app_name>-operator`
    /// * `app.kubernetes.io/name = <app_name>`
    pub async fn add_configmap(
        &mut self,
        client: &Client,
        configmap: &ConfigMap,
    ) -> OperatorResult<ConfigMap> {
        self.check_labels(configmap.labels())?;

        let patched_configmap = self.patch_resource(client, configmap).await?;
        self.configmaps.insert(&patched_configmap);

        Ok(patched_configmap)
    }

    /// Adds a stateful set to the cluster resources.
    ///
    /// The stateful set will be patched and the patched resource will be returned.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `statefulset` - A stateful set to add to the cluster
    ///
    /// # Errors
    ///
    /// If the labels of the given stateful set are not set properly then an `Error::MissingLabel`
    /// or `Error::UnexpectedLabelContent` is returned. The expected labels are:
    /// * `app.kubernetes.io/instance = <cluster.name>`
    /// * `app.kubernetes.io/managed-by = <app_name>-operator`
    /// * `app.kubernetes.io/name = <app_name>`
    pub async fn add_statefulset(
        &mut self,
        client: &Client,
        statefulset: &StatefulSet,
    ) -> OperatorResult<StatefulSet> {
        self.check_labels(statefulset.labels())?;

        let patched_statefulset = self.patch_resource(client, statefulset).await?;
        self.statefulsets.insert(&patched_statefulset);

        Ok(patched_statefulset)
    }

    fn check_labels(&self, labels: &BTreeMap<String, String>) -> OperatorResult<()> {
        ClusterResources::check_label(labels, APP_INSTANCE_LABEL, &self.app_instance)?;
        ClusterResources::check_label(labels, APP_MANAGED_BY_LABEL, &self.manager)?;
        ClusterResources::check_label(labels, APP_NAME_LABEL, &self.app_name)?;
        Ok(())
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

    /// Patches the given resource.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `resource` - The resource to patch
    async fn patch_resource<T>(&self, client: &Client, resource: &T) -> OperatorResult<T>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Serialize,
    {
        let patched_resource = client
            .apply_patch(&self.manager, resource, resource)
            .await?;

        Ok(patched_resource)
    }

    /// Finalizes the cluster creation.
    ///
    /// All orphaned resources, i.e. resources which are labelled as if they belong to this cluster
    /// instance but were not added to the cluster resources, are deleted.
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
        self.delete_orphaned_resources(client, &self.services)
            .await?;
        self.delete_orphaned_resources(client, &self.statefulsets)
            .await?;
        self.delete_orphaned_resources(client, &self.configmaps)
            .await?;

        Ok(())
    }

    /// Deletes all deployed resources which are labelled as if they belong to this cluster
    /// instance but are not contained in the given list.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    /// * `desired_resources` - The resources to keep
    async fn delete_orphaned_resources<T>(
        &self,
        client: &Client,
        desired_resources: &ResourceSet<T>,
    ) -> OperatorResult<()>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
    {
        let deployed_cluster_resources = self.list_deployed_cluster_resources::<T>(client).await?;

        let orphaned_resources = deployed_cluster_resources.subtract(desired_resources);

        if !orphaned_resources.is_empty() {
            info!(
                "Deleting orphaned {}: {}",
                T::plural(&()),
                orphaned_resources
            );
            for resource in orphaned_resources.iter() {
                client.delete(resource).await?;
            }
        }

        Ok(())
    }

    /// Lists the deployed resources with instance, name, and managed-by labels equal to this
    /// cluster instance.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    async fn list_deployed_cluster_resources<T>(
        &self,
        client: &Client,
    ) -> OperatorResult<ResourceSet<T>>
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

        Ok(resources.as_slice().into())
    }
}

/// Set of resources
///
/// Resources are seen as equal if their resource IDs are identical.
#[derive(Debug, Default)]
struct ResourceSet<T>(HashMap<ResourceId, T>);

impl<T> Eq for ResourceSet<T> {}

impl<T> PartialEq for ResourceSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.keys().eq(other.0.keys())
    }
}

impl<T> From<&[T]> for ResourceSet<T>
where
    T: Clone + Resource<DynamicType = ()>,
{
    fn from(resources: &[T]) -> Self {
        Self(
            resources
                .iter()
                .map(|r| (ResourceId::from(r), r.to_owned()))
                .collect(),
        )
    }
}

impl<T> Display for ResourceSet<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut resource_id_strings = self.0.keys().map(ResourceId::to_string).collect::<Vec<_>>();
        resource_id_strings.sort();
        write!(f, "{}", resource_id_strings.join(", "))
    }
}

impl<T> ResourceSet<T>
where
    T: Clone + Resource<DynamicType = ()>,
{
    /// Returns true if the resource set is empty, false otherwise.
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true if the set contains the given resource, false otherwise.
    fn contains(&self, resource: &T) -> bool {
        self.0.contains_key(&resource.into())
    }

    /// Inserts the given resource into the resource set.
    ///
    /// The resource is updated if it is already contained in the set.
    fn insert(&mut self, resource: &T) {
        self.0.insert(resource.into(), resource.to_owned());
    }

    /// Returns the difference of this resource set and the given one.
    ///
    /// The resources are compared by their resource ID. The result contains the resources from
    /// this set.
    fn subtract(&self, other: &ResourceSet<T>) -> ResourceSet<T> {
        self.iter()
            .filter(|resource| !other.contains(resource))
            .cloned()
            .collect::<Vec<_>>()
            .as_slice()
            .into()
    }

    /// Returns an iterator over the resources contained in this set.
    fn iter(&self) -> Values<'_, ResourceId, T> {
        self.0.values()
    }
}

/// A resource ID solely consisting of namespace and name.
#[derive(Debug, Eq, Hash, PartialEq)]
struct ResourceId {
    namespace: Option<String>,
    name: String,
}

impl<T> From<&T> for ResourceId
where
    T: Resource<DynamicType = ()>,
{
    fn from(resource: &T) -> Self {
        Self {
            namespace: resource.namespace(),
            name: resource.name(),
        }
    }
}

impl Display for ResourceId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(namespace) = &self.namespace {
            write!(f, ".{}", namespace)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::{Node, Pod};
    use kube::core::ObjectMeta;

    mod cluster_resources_tests {
        use super::super::*;

        #[test]
        fn cluster_resources_can_be_created_from_valid_parameters() {
            let cluster = ObjectReference {
                name: Some("appcluster".into()),
                namespace: Some("default".into()),
                ..Default::default()
            };

            let result = ClusterResources::new("app", "appcluster_scope", &cluster);

            assert!(result.is_ok());
        }
    }

    mod resourceset_tests {
        use super::super::*;
        use super::*;
        use k8s_openapi::api::core::v1::{Pod, PodStatus};

        #[test]
        fn resourceset_are_equal_up_to_the_resource_ids() {
            let resource1a = Pod {
                status: None,
                ..create_namespaced_resource("namespace", "resource1")
            };
            let resource1b = Pod {
                status: Some(PodStatus::default()),
                ..create_namespaced_resource("namespace", "resource1")
            };
            let resource2 = create_namespaced_resource("namespace", "resource2");

            let resourceset1a = ResourceSet::from([resource1a].as_ref());
            let resourceset1b = ResourceSet::from([resource1b].as_ref());
            let resourceset2 = ResourceSet::from([resource2].as_ref());

            assert_eq!(resourceset1a, resourceset1b);
            assert_ne!(resourceset1a, resourceset2);
        }

        #[test]
        fn resourceset_can_be_displayed() {
            let resource1 = create_namespaced_resource("namespace", "resource1");
            let resource2 = create_namespaced_resource("namespace", "resource2");

            let resourceset1 = ResourceSet::from(Vec::<Pod>::new().as_ref());
            let resourceset2 = ResourceSet::from([resource1.to_owned()].as_ref());
            let resourceset3 = ResourceSet::from([resource1, resource2].as_ref());

            assert_eq!("", resourceset1.to_string());
            assert_eq!("resource1.namespace", resourceset2.to_string());
            assert_eq!(
                "resource1.namespace, resource2.namespace",
                resourceset3.to_string()
            );
        }

        #[test]
        fn resourceset_can_be_checked_for_emptiness() {
            let resource1 = create_namespaced_resource("namespace", "resource1");

            let resourceset1 = ResourceSet::from(Vec::<Pod>::new().as_ref());
            let resourceset2 = ResourceSet::from([resource1].as_ref());

            assert!(resourceset1.is_empty());
            assert!(!resourceset2.is_empty());
        }

        #[test]
        fn resourceset_contains_expected_resources() {
            let resource1 = create_namespaced_resource("namespace", "resource1");
            let resource2 = create_namespaced_resource("namespace", "resource2");
            let resource3 = create_namespaced_resource("namespace", "resource3");

            let resourceset =
                ResourceSet::from([resource1.to_owned(), resource2.to_owned()].as_ref());

            assert!(resourceset.contains(&resource1));
            assert!(resourceset.contains(&resource2));
            assert!(!resourceset.contains(&resource3));
        }

        #[test]
        fn set_difference_of_two_resourcesets_can_be_built() {
            let resource1 = create_namespaced_resource("namespace", "resource1");
            let resource2 = create_namespaced_resource("namespace", "resource2");
            let resource3 = create_namespaced_resource("namespace", "resource3");

            let resourceset1 =
                ResourceSet::from([resource1.to_owned(), resource2.to_owned()].as_ref());
            let resourceset2 =
                ResourceSet::from([resource2.to_owned(), resource3.to_owned()].as_ref());

            let set_difference = resourceset1.subtract(&resourceset2);

            assert!(set_difference.contains(&resource1));
            assert!(!set_difference.contains(&resource2));
            assert!(!set_difference.contains(&resource3));
        }

        #[test]
        fn resourcesets_are_iterable() {
            let resource = create_namespaced_resource("namespace", "resource");

            let resourceset = ResourceSet::from([resource.to_owned()].as_ref());

            let next_resource = resourceset.iter().next();

            assert_eq!(Some(&resource), next_resource);
        }
    }

    mod resourceid_tests {
        use super::super::*;
        use super::*;

        #[test]
        fn display_namespaced_resourceid() {
            let resource = create_namespaced_resource("namespace", "name");

            let resource_id = ResourceId::from(&resource);

            assert_eq!("name.namespace", resource_id.to_string());
        }

        #[test]
        fn display_non_namespaced_resourceid() {
            let resource = create_non_namespaced_resource("name");

            let resource_id = ResourceId::from(&resource);

            assert_eq!("name", resource_id.to_string());
        }
    }

    fn create_namespaced_resource(namespace: &str, name: &str) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                namespace: Some(namespace.into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_non_namespaced_resource(name: &str) -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
