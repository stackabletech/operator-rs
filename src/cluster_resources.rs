//! A structure containing the cluster resources.

use std::{
    collections::HashSet,
    fmt::{self, Debug, Display, Formatter},
};

use crate::{
    client::Client,
    error::OperatorResult,
    k8s_openapi::{
        api::{
            apps::v1::StatefulSet,
            core::v1::{ConfigMap, ObjectReference, Service},
        },
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    },
    kube::{Resource, ResourceExt},
    labels::{self, APP_INSTANCE_LABEL, APP_MANAGED_BY_LABEL, APP_NAME_LABEL},
};
use serde::{de::DeserializeOwned, Serialize};
use tracing::info;

/// A structure containing the cluster resources.
///
/// The cluster resources can be updated which means that changed resources are patched and
/// orphaned ones are deleted. A cluster resource becomes orphaned if a role or role group is
/// removed from a cluster specification.
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
///     UpdateClusterResources {
///         source: stackable_operator::error::Error,
///     },
/// };
///
/// async fn reconcile(app: Arc<AppCluster>, client: Arc<Client>) -> Result<Action, Error> {
///     let validated_config = ValidatedRoleConfigByPropertyKind::default();
///
///     let mut cluster_services = Vec::new();
///     let mut cluster_configmaps = Vec::new();
///     let mut cluster_statefulsets = Vec::new();
///
///     let role_service = Service::default();
///     cluster_services.push(role_service);
///
///     let discovery_configmap = ConfigMap::default();
///     cluster_configmaps.push(discovery_configmap);
///
///     for (role_name, group_config) in validated_config.iter() {
///         for (rolegroup_name, rolegroup_config) in group_config.iter() {
///             let rolegroup_service = Service::default();
///             cluster_services.push(rolegroup_service);
///
///             let rolegroup_configmap = ConfigMap::default();
///             cluster_configmaps.push(rolegroup_configmap);
///
///             let rolegroup_statefulset = StatefulSet::default();
///             cluster_statefulsets.push(rolegroup_statefulset);
///         }
///     }
///
///     ClusterResources::new(
///         APP_NAME,
///         FIELD_MANAGER_SCOPE,
///         &app.object_ref(&()),
///         &cluster_services,
///         &cluster_configmaps,
///         &cluster_statefulsets,
///     )
///     .update(&client)
///     .await
///     .map_err(|source| Error::UpdateClusterResources { source })?;
///
///     Ok(Action::await_change())
/// }
/// ```
pub struct ClusterResources {
    namespace: String,
    app_instance: String,
    app_name: String,
    app_managed_by: String,
    field_manager_scope: String,
    services: Vec<Service>,
    configmaps: Vec<ConfigMap>,
    statefulsets: Vec<StatefulSet>,
}

impl ClusterResources {
    /// Constructs new `ClusterResources`.
    ///
    /// # Arguments
    ///
    /// * `app_name` - The lower-case application name used in the resource labels, e.g.
    ///   "zookeeper"
    /// * `field_manager_scope` - The field manager scope used for patching the resources, e.g.
    ///   "zookeepercluster"
    /// * `cluster` - A reference to the cluster containing the name and namespace of the cluster
    /// * `services` - All services the cluster consists of; Deployed services which are not
    ///    included in this list, are considered orphaned and deleted when `update` is called.
    /// * `configmaps` - All config maps the cluster consists of; Deployed config maps which are
    ///    not included in this list, are considered orphaned and deleted when `update` is called.
    /// * `statefulsets` - All stateful sets the cluster consists of; Deployed stateful sets which
    ///    are not included in this list, are considered orphaned and deleted when `update` is
    ///    called.
    pub fn new(
        app_name: &str,
        field_manager_scope: &str,
        cluster: &ObjectReference,
        services: &[Service],
        configmaps: &[ConfigMap],
        statefulsets: &[StatefulSet],
    ) -> Self {
        ClusterResources {
            namespace: cluster
                .namespace
                .as_ref()
                .expect("Cluster namespace expected")
                .to_owned(),
            app_instance: cluster
                .name
                .as_ref()
                .expect("Cluster name expected")
                .to_owned(),
            app_name: app_name.into(),
            app_managed_by: labels::get_app_managed_by_value(app_name),
            field_manager_scope: field_manager_scope.into(),
            services: services.into(),
            configmaps: configmaps.into(),
            statefulsets: statefulsets.into(),
        }
    }

    /// Updates the cluster according to the resources given in this structure.
    ///
    /// The given resources are patched and all orphaned resources, i.e. resources which are
    /// labelled as if they belong to this cluster instance but are not contained in the given
    /// resources, are deleted.
    ///
    /// The following resource labels are compared:
    /// * `app.kubernetes.io/instance`
    /// * `app.kubernetes.io/managed-by`
    /// * `app.kubernetes.io/name`
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    pub async fn update(&self, client: &Client) -> OperatorResult<()> {
        self.patch_resources(client, &self.configmaps).await?;
        self.patch_resources(client, &self.statefulsets).await?;
        self.patch_resources(client, &self.services).await?;

        self.delete_orphaned_resources(client, &self.services)
            .await?;
        self.delete_orphaned_resources(client, &self.statefulsets)
            .await?;
        self.delete_orphaned_resources(client, &self.configmaps)
            .await?;

        Ok(())
    }

    /// Patches the given resources.
    ///
    /// # Arguments
    ///
    /// * `client` - The client which is used to access Kubernetes
    async fn patch_resources<T>(&self, client: &Client, resources: &[T]) -> OperatorResult<()>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Serialize,
    {
        for resource in resources {
            client
                .apply_patch(&self.field_manager_scope, resource, resource)
                .await?;
        }

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
        desired_resources: &[T],
    ) -> OperatorResult<()>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
    {
        let deployed_cluster_resources = self.list_deployed_cluster_resources::<T>(client).await?;

        let desired_resource_id_set: ResourceIdSet = desired_resources.into();

        let orphaned_resources = deployed_cluster_resources
            .into_iter()
            .filter(|deployed_resource| !desired_resource_id_set.contains(deployed_resource))
            .collect::<Vec<_>>();

        if !orphaned_resources.is_empty() {
            info!(
                "Deleting orphaned {}: {}",
                T::plural(&()),
                ResourceIdSet::from(orphaned_resources.as_ref())
            );
            for resource in &orphaned_resources {
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
                    values: Some(vec![self.app_managed_by.to_owned()]),
                },
            ]),
            ..Default::default()
        };

        client
            .list_with_label_selector(Some(&self.namespace), &label_selector)
            .await
    }
}

/// Set of resource IDs
struct ResourceIdSet(HashSet<ResourceId>);

impl<T> From<&[T]> for ResourceIdSet
where
    T: Resource<DynamicType = ()>,
{
    fn from(resources: &[T]) -> Self {
        Self(resources.iter().map(ResourceId::from).collect())
    }
}

impl Display for ResourceIdSet {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut resource_id_strings = self.0.iter().map(ResourceId::to_string).collect::<Vec<_>>();
        resource_id_strings.sort();
        write!(f, "{}", resource_id_strings.join(", "))
    }
}

impl ResourceIdSet {
    /// Returns true if the set contains the given resource, false otherwise.
    fn contains<T>(&self, resource: &T) -> bool
    where
        T: Resource<DynamicType = ()>,
    {
        self.0.contains(&resource.into())
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
    use super::*;
    use k8s_openapi::api::core::v1::{Node, Pod};
    use kube::core::ObjectMeta;

    #[test]
    fn check_content_of_resourceidset() {
        let resource1 = create_namespaced_resource("namespace", "resource1");
        let resource2 = create_namespaced_resource("namespace", "resource2");
        let resource3 = create_namespaced_resource("namespace", "resource3");

        let resource_id_set =
            ResourceIdSet::from(vec![resource1.to_owned(), resource2.to_owned()].as_ref());

        assert!(resource_id_set.contains(&resource1));
        assert!(resource_id_set.contains(&resource2));
        assert!(!resource_id_set.contains(&resource3));
    }

    #[test]
    fn display_resourceidset() {
        let resource1 = create_namespaced_resource("namespace", "resource1");
        let resource2 = create_namespaced_resource("namespace", "resource2");

        let resource_id_set1 = ResourceIdSet::from(Vec::<Pod>::new().as_ref());
        let resource_id_set2 = ResourceIdSet::from(vec![resource1.to_owned()].as_ref());
        let resource_id_set3 = ResourceIdSet::from(vec![resource1, resource2].as_ref());

        assert_eq!("", resource_id_set1.to_string());
        assert_eq!("resource1.namespace", resource_id_set2.to_string());
        assert_eq!(
            "resource1.namespace, resource2.namespace",
            resource_id_set3.to_string()
        );
    }

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
