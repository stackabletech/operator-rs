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
///     CreateClusterResources {
///         source: stackable_operator::error::Error,
///     },
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
///     cluster_configmaps.push(discovery_configmap.clone());
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
///     let mut cluster_resources = ClusterResources::new(
///         APP_NAME,
///         FIELD_MANAGER_SCOPE,
///         &app.object_ref(&()),
///         &cluster_services,
///         &cluster_configmaps,
///         &cluster_statefulsets,
///     )
///     .map_err(|source| Error::CreateClusterResources { source })?;
///
///     cluster_resources
///         .update(&client)
///         .await
///         .map_err(|source| Error::UpdateClusterResources { source })?;
///
///     // Updated resources can be retrieved, e.g. to create a discovery hash.
///     let updated_discovery_map = cluster_resources
///         .get_configmap(&discovery_configmap)
///         .unwrap();
///
///     Ok(Action::await_change())
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ClusterResources {
    namespace: String,
    app_instance: String,
    app_name: String,
    app_managed_by: String,
    field_manager_scope: String,
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
    ///
    /// # Errors
    ///
    /// If `cluster` does not contain a namespace and a name then an `Error::MissingObjectKey` is
    /// returned.
    ///
    /// If the labels of the given resources are not set properly then an `Error::MissingLabel` or
    /// `Error::UnexpectedLabelContent` is returned. The expected labels are:
    /// * `app.kubernetes.io/instance = <cluster.name>`
    /// * `app.kubernetes.io/managed-by = <app_name>-operator`
    /// * `app.kubernetes.io/name = <app_name>`
    ///
    pub fn new(
        app_name: &str,
        field_manager_scope: &str,
        cluster: &ObjectReference,
        services: &[Service],
        configmaps: &[ConfigMap],
        statefulsets: &[StatefulSet],
    ) -> OperatorResult<Self> {
        let namespace = cluster
            .namespace
            .to_owned()
            .ok_or(Error::MissingObjectKey { key: "namespace" })?;
        let app_instance = cluster
            .name
            .to_owned()
            .ok_or(Error::MissingObjectKey { key: "name" })?;
        let app_managed_by = labels::get_app_managed_by_value(app_name);

        let check_labels = |labels| -> OperatorResult<()> {
            ClusterResources::check_label(labels, APP_INSTANCE_LABEL, &app_instance)?;
            ClusterResources::check_label(labels, APP_MANAGED_BY_LABEL, &app_managed_by)?;
            ClusterResources::check_label(labels, APP_NAME_LABEL, app_name)?;
            Ok(())
        };

        services
            .iter()
            .map(ResourceExt::labels)
            .try_for_each(check_labels)?;
        configmaps
            .iter()
            .map(ResourceExt::labels)
            .try_for_each(check_labels)?;
        statefulsets
            .iter()
            .map(ResourceExt::labels)
            .try_for_each(check_labels)?;

        Ok(ClusterResources {
            namespace,
            app_instance,
            app_name: app_name.into(),
            app_managed_by,
            field_manager_scope: field_manager_scope.into(),
            services: services.into(),
            configmaps: configmaps.into(),
            statefulsets: statefulsets.into(),
        })
    }

    /// Checks that the given `labels` contain the given `label` with the given `expected_content`.
    ///
    /// # Errors
    ///
    /// If `labels` does not contain `label` then an `Error::MissingLabel` is returned.
    ///
    /// If `labels` contains the given `label` but not with the `expected_content` then an
    /// `Error::UnexpectedLabelContent` is returned
    ///
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

    /// Returns the updated version of the given service.
    pub fn get_service(&self, service: &Service) -> Option<Service> {
        self.services.get(service)
    }

    /// Returns the updated version of the given config map.
    pub fn get_configmap(&self, configmap: &ConfigMap) -> Option<ConfigMap> {
        self.configmaps.get(configmap)
    }

    /// Returns the updated version of the given stateful set.
    pub fn get_statefulset(&self, statefulset: &StatefulSet) -> Option<StatefulSet> {
        self.statefulsets.get(statefulset)
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
    pub async fn update(&mut self, client: &Client) -> OperatorResult<()> {
        self.configmaps = self
            .patch_resources(client, &self.configmaps)
            .await?
            .as_slice()
            .into();
        self.statefulsets = self
            .patch_resources(client, &self.statefulsets)
            .await?
            .as_slice()
            .into();
        self.services = self
            .patch_resources(client, &self.services)
            .await?
            .as_slice()
            .into();

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
    async fn patch_resources<T>(
        &self,
        client: &Client,
        resources: &ResourceSet<T>,
    ) -> OperatorResult<Vec<T>>
    where
        T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Serialize,
    {
        let mut patched_resources = Vec::new();

        for resource in resources.iter() {
            let patched_resource = client
                .apply_patch(&self.field_manager_scope, resource, resource)
                .await?;
            patched_resources.push(patched_resource);
        }

        Ok(patched_resources)
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
                    values: Some(vec![self.app_managed_by.to_owned()]),
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

/// Set of resource IDs
#[derive(Debug)]
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
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true if the set contains the given resource, false otherwise.
    fn contains(&self, resource: &T) -> bool {
        self.0.contains_key(&resource.into())
    }

    /// Returns the resource with the same resource ID as the given one.
    fn get(&self, resource: &T) -> Option<T> {
        self.0.get(&ResourceId::from(resource)).cloned()
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
        use kube::core::ObjectMeta;

        #[test]
        fn cluster_resources_can_be_created_from_valid_parameters() {
            let cluster = ObjectReference {
                name: Some("appcluster".into()),
                namespace: Some("default".into()),
                ..Default::default()
            };

            let metadata_template = ObjectMeta {
                labels: Some(
                    [
                        ("app.kubernetes.io/instance".into(), "appcluster".into()),
                        ("app.kubernetes.io/name".into(), "app".into()),
                        ("app.kubernetes.io/managed-by".into(), "app-operator".into()),
                    ]
                    .into(),
                ),
                ..Default::default()
            };
            let service = Service {
                metadata: ObjectMeta {
                    name: Some("service".into()),
                    ..metadata_template.to_owned()
                },
                ..Default::default()
            };
            let configmap = ConfigMap {
                metadata: ObjectMeta {
                    name: Some("configmap".into()),
                    ..metadata_template.to_owned()
                },
                ..Default::default()
            };
            let statefulset = StatefulSet {
                metadata: ObjectMeta {
                    name: Some("statefulset".into()),
                    ..metadata_template
                },
                ..Default::default()
            };

            let cluster_resources = ClusterResources::new(
                "app",
                "appcluster_scope",
                &cluster,
                &[service.to_owned()],
                &[configmap.to_owned()],
                &[statefulset.to_owned()],
            )
            .expect("no error");

            assert_eq!(
                ClusterResources {
                    namespace: "default".into(),
                    app_instance: "appcluster".into(),
                    app_name: "app".into(),
                    app_managed_by: "app-operator".into(),
                    field_manager_scope: "appcluster_scope".into(),
                    services: [service].as_ref().into(),
                    configmaps: [configmap].as_ref().into(),
                    statefulsets: [statefulset].as_ref().into(),
                },
                cluster_resources
            );
        }

        #[test]
        fn error_is_returned_when_label_is_missing() {
            let cluster = ObjectReference {
                name: Some("appcluster".into()),
                namespace: Some("default".into()),
                ..Default::default()
            };

            let service = Service {
                metadata: ObjectMeta {
                    name: Some("service".into()),
                    labels: Some(
                        [
                            ("app.kubernetes.io/name".into(), "app".into()),
                            ("app.kubernetes.io/managed-by".into(), "app-operator".into()),
                        ]
                        .into(),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            };

            let result =
                ClusterResources::new("app", "appcluster_scope", &cluster, &[service], &[], &[]);

            match result {
                Err(Error::MissingLabel { label }) => {
                    assert_eq!("app.kubernetes.io/instance", label);
                }
                _ => panic!("Error::MissingLabel expected"),
            }
        }

        #[test]
        fn error_is_returned_when_label_content_is_wrong() {
            let cluster = ObjectReference {
                name: Some("appcluster".into()),
                namespace: Some("default".into()),
                ..Default::default()
            };

            let service = Service {
                metadata: ObjectMeta {
                    name: Some("service".into()),
                    labels: Some(
                        [
                            ("app.kubernetes.io/instance".into(), "anothercluster".into()),
                            ("app.kubernetes.io/name".into(), "app".into()),
                            ("app.kubernetes.io/managed-by".into(), "app-operator".into()),
                        ]
                        .into(),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            };

            let result =
                ClusterResources::new("app", "appcluster_scope", &cluster, &[service], &[], &[]);

            match result {
                Err(Error::UnexpectedLabelContent {
                    label,
                    expected_content,
                    actual_content,
                }) => {
                    assert_eq!("app.kubernetes.io/instance", label);
                    assert_eq!("appcluster", expected_content);
                    assert_eq!("anothercluster", actual_content);
                }
                _ => panic!("Error::UnexpectedLabelContent expected"),
            }
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
        fn resource_can_be_retrieved_from_resourceset() {
            let resource1a = Pod {
                status: None,
                ..create_namespaced_resource("namespace", "resource1")
            };
            let resource1b = Pod {
                status: Some(PodStatus::default()),
                ..create_namespaced_resource("namespace", "resource1")
            };

            let resourceset = ResourceSet::from([resource1a.to_owned()].as_ref());

            let resource_from_resourceset = resourceset.get(&resource1b);

            assert_eq!(Some(resource1a), resource_from_resourceset);
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
