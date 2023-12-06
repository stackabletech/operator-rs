//! A structure containing the cluster resources.

use crate::{
    client::{Client, GetApi},
    commons::{
        cluster_operation::ClusterOperation,
        resources::{
            ComputeResource, ResourceRequirementsExt, ResourceRequirementsType,
            LIMIT_REQUEST_RATIO_CPU, LIMIT_REQUEST_RATIO_MEMORY,
        },
    },
    error::{Error, OperatorResult},
    labels::{APP_INSTANCE_LABEL, APP_MANAGED_BY_LABEL, APP_NAME_LABEL},
    utils::format_full_controller_name,
};

use k8s_openapi::{
    api::{
        apps::v1::{DaemonSet, DaemonSetSpec, StatefulSet, StatefulSetSpec},
        batch::v1::Job,
        core::v1::{
            ConfigMap, ObjectReference, PodSpec, PodTemplateSpec, Secret, Service, ServiceAccount,
        },
        policy::v1::PodDisruptionBudget,
        rbac::v1::RoleBinding,
    },
    apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    NamespaceResourceScope,
};
use kube::{core::ErrorResponse, Resource, ResourceExt};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
};
use strum::Display;
use tracing::{debug, info, warn};

#[cfg(doc)]
use crate::k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{NodeSelector, Pod},
};

/// A cluster resource handled by [`ClusterResources`].
///
/// This trait is used in the function signatures of [`ClusterResources`] and restricts the
/// possible kinds of resources. [`ClusterResources::delete_orphaned_resources`] iterates over all
/// implementations and removes the orphaned resources. Therefore if a new implementation is added,
/// it must be added to [`ClusterResources::delete_orphaned_resources`] as well.
pub trait ClusterResource:
    Clone
    + Debug
    + DeserializeOwned
    + Resource<DynamicType = (), Scope = NamespaceResourceScope>
    + GetApi<Namespace = str>
    + Serialize
{
    /// This must be implemented for any [`ClusterResources`] that should be adapted before
    /// applying depending on the chosen [`ClusterResourceApplyStrategy`].
    /// An example would be setting [`StatefulSet`] replicas to 0 for the
    /// `ClusterResourceApplyStrategy::ClusterStopped`.
    fn maybe_mutate(self, _strategy: &ClusterResourceApplyStrategy) -> Self {
        self
    }

    fn pod_spec(&self) -> Option<&PodSpec> {
        None
    }
}

/// The [`ClusterResourceApplyStrategy`] defines how to handle resources applied by the operators.
/// This can be default behavior (apply_patch), only retrieving resources (get) for cluster status
/// purposes or doing nothing.
#[derive(Debug, Display, Eq, PartialEq)]
pub enum ClusterResourceApplyStrategy {
    /// Default strategy. Resources a applied via the [`Client::apply_patch`] client method.
    Default,
    /// Strategy to pause reconciliation. This means any cluster changes (e.g. in the custom resource
    /// spec) are ignored. Resources are not applied at all but merely fetched via the
    /// [`Client::get`] method in order to still be able to e.g. update the cluster status.
    ReconciliationPaused,
    /// Strategy to stop the cluster. This means no Pods should be scheduled for either [`StatefulSet`],
    /// [`DaemonSet`] or [`Deployment`]. This is done by setting the [`StatefulSet`] or [`Deployment`]
    /// replicas to 0. For [`DaemonSet`] this is done via an unreachable [`NodeSelector`].
    ///
    /// Resources are applied via the [`Client::apply_patch`] client method.
    ClusterStopped,
    /// Dry-run strategy that doesn't actually create any workload resources. This is useful for
    /// Superset and Airflow clusters that need to wait for their databases to be set up first.
    NoApply,
}

impl From<&ClusterOperation> for ClusterResourceApplyStrategy {
    fn from(commons_spec: &ClusterOperation) -> Self {
        if commons_spec.reconciliation_paused {
            ClusterResourceApplyStrategy::ReconciliationPaused
        } else if commons_spec.stopped {
            ClusterResourceApplyStrategy::ClusterStopped
        } else {
            ClusterResourceApplyStrategy::Default
        }
    }
}

impl ClusterResourceApplyStrategy {
    /// Interacts with the API server depending on the strategy.
    /// This can be applying, listing resources or doing nothing.
    async fn run<T: ClusterResource + Sync>(
        &self,
        manager: &str,
        resource: &T,
        client: &Client,
    ) -> OperatorResult<T> {
        match self {
            Self::ReconciliationPaused => {
                debug!(
                    "Get resource [{}] because of [{}] strategy.",
                    resource.name_any(),
                    self
                );
                client
                    .get(
                        &resource.name_any(),
                        resource
                            .namespace()
                            .as_deref()
                            .ok_or(Error::MissingObjectKey { key: "namespace" })?,
                    )
                    .await
            }
            Self::Default | Self::ClusterStopped => {
                debug!(
                    "Patching resource [{}] because of [{}] strategy.",
                    resource.name_any(),
                    self
                );
                client.apply_patch(manager, resource, resource).await
            }
            Self::NoApply => {
                debug!(
                    "Skip creating resource [{}] because of [{}] strategy.",
                    resource.name_any(),
                    self
                );
                Ok(resource.clone())
            }
        }
    }

    /// Indicates if orphaned resources should be deleted depending on the strategy.
    const fn delete_orphans(&self) -> bool {
        match self {
            ClusterResourceApplyStrategy::NoApply
            | ClusterResourceApplyStrategy::ReconciliationPaused => false,
            ClusterResourceApplyStrategy::ClusterStopped
            | ClusterResourceApplyStrategy::Default => true,
        }
    }
}

// IMPORTANT: Don't forget to add new Resources to [`delete_orphaned_resources`] as well!
impl ClusterResource for ConfigMap {}
impl ClusterResource for Secret {}
impl ClusterResource for Service {}
impl ClusterResource for ServiceAccount {}
impl ClusterResource for RoleBinding {}
impl ClusterResource for PodDisruptionBudget {}

impl ClusterResource for Job {
    fn pod_spec(&self) -> Option<&PodSpec> {
        self.spec
            .as_ref()
            .and_then(|spec| spec.template.spec.as_ref())
    }
}

impl ClusterResource for StatefulSet {
    fn maybe_mutate(self, strategy: &ClusterResourceApplyStrategy) -> Self {
        match strategy {
            ClusterResourceApplyStrategy::ClusterStopped => StatefulSet {
                spec: Some(StatefulSetSpec {
                    replicas: Some(0),
                    ..self.spec.unwrap_or_default()
                }),
                ..self
            },
            ClusterResourceApplyStrategy::Default
            | ClusterResourceApplyStrategy::ReconciliationPaused
            | ClusterResourceApplyStrategy::NoApply => self,
        }
    }

    fn pod_spec(&self) -> Option<&PodSpec> {
        self.spec
            .as_ref()
            .and_then(|spec| spec.template.spec.as_ref())
    }
}

impl ClusterResource for DaemonSet {
    fn maybe_mutate(self, strategy: &ClusterResourceApplyStrategy) -> Self {
        match strategy {
            ClusterResourceApplyStrategy::ClusterStopped => DaemonSet {
                spec: Some(DaemonSetSpec {
                    template: PodTemplateSpec {
                        spec: Some(PodSpec {
                            node_selector: Some(
                                [(
                                    "stackable.tech/do-not-schedule".to_string(),
                                    "cluster-stopped".to_string(),
                                )]
                                .into_iter()
                                .collect::<BTreeMap<String, String>>(),
                            ),
                            ..self
                                .spec
                                .clone()
                                .unwrap_or_default()
                                .template
                                .spec
                                .unwrap_or_default()
                        }),
                        ..self.spec.clone().unwrap_or_default().template
                    },
                    ..self.spec.unwrap_or_default()
                }),
                ..self
            },
            ClusterResourceApplyStrategy::Default
            | ClusterResourceApplyStrategy::ReconciliationPaused
            | ClusterResourceApplyStrategy::NoApply => self,
        }
    }

    fn pod_spec(&self) -> Option<&PodSpec> {
        self.spec
            .as_ref()
            .and_then(|spec| spec.template.spec.as_ref())
    }
}

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
/// use stackable_operator::cluster_resources::{ClusterResourceApplyStrategy, ClusterResources};
/// use stackable_operator::product_config_utils::ValidatedRoleConfigByPropertyKind;
/// use stackable_operator::role_utils::Role;
/// use std::sync::Arc;
///
/// const APP_NAME: &str = "app";
/// const OPERATOR_NAME: &str = "app.stackable.tech";
/// const CONTROLLER_NAME: &str = "appcluster";
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
///         OPERATOR_NAME,
///         CONTROLLER_NAME,
///         &app.object_ref(&()),
///         ClusterResourceApplyStrategy::Default,
///     )
///     .map_err(|source| Error::CreateClusterResources { source })?;
///
///     let role_service = Service::default();
///     let patched_role_service =
///         cluster_resources.add(&client, role_service)
///             .await
///             .map_err(|source| Error::AddClusterResource { source })?;
///
///     for (role_name, group_config) in validated_config.iter() {
///         for (rolegroup_name, rolegroup_config) in group_config.iter() {
///             let rolegroup_service = Service::default();
///             cluster_resources.add(&client, rolegroup_service)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_configmap = ConfigMap::default();
///             cluster_resources.add(&client, rolegroup_configmap)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///
///             let rolegroup_statefulset = StatefulSet::default();
///             cluster_resources.add(&client, rolegroup_statefulset)
///                 .await
///                 .map_err(|source| Error::AddClusterResource { source })?;
///         }
///     }
///
///     let discovery_configmap = ConfigMap::default();
///     let patched_discovery_configmap =
///         cluster_resources.add(&client, discovery_configmap)
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
    /// Strategy to manage how cluster resources are applied. Resources could be patched, merged
    /// or not applied at all depending on the strategy.
    apply_strategy: ClusterResourceApplyStrategy,
}

impl ClusterResources {
    /// Constructs new `ClusterResources`.
    ///
    /// # Arguments
    ///
    /// * `app_name` - The lower-case application name used in the resource labels, e.g.
    ///   "zookeeper"
    /// * `operator_name` - The FQDN-style name of the operator, such as ""zookeeper.stackable.tech""
    /// * `controller_name` - The name of the lower-case name of the primary resource that
    ///   the controller manages, such as "zookeepercluster"
    /// * `cluster` - A reference to the cluster containing the name and namespace of the cluster
    /// * `apply_strategy` - A strategy to manage how cluster resources are applied to the API server
    ///
    /// The combination of (`operator_name`, `controller_name`) must be unique for each controller in the cluster,
    /// otherwise resources created by another controller are detected as orphaned and deleted.
    ///
    /// # Errors
    ///
    /// If `cluster` does not contain a namespace and a name then an `Error::MissingObjectKey` is
    /// returned.
    pub fn new(
        app_name: &str,
        operator_name: &str,
        controller_name: &str,
        cluster: &ObjectReference,
        apply_strategy: ClusterResourceApplyStrategy,
    ) -> OperatorResult<Self> {
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
            manager: format_full_controller_name(operator_name, controller_name),
            resource_ids: Default::default(),
            apply_strategy,
        })
    }

    /// Return required labels for cluster resources to be uniquely identified for clean up.
    // TODO: This is a (quick-fix) helper method but should be replaced by better label handling
    pub fn get_required_labels(&self) -> BTreeMap<String, String> {
        vec![
            (
                APP_INSTANCE_LABEL.to_string(),
                self.app_instance.to_string(),
            ),
            (APP_MANAGED_BY_LABEL.to_string(), self.manager.to_string()),
            (APP_NAME_LABEL.to_string(), self.app_name.to_string()),
        ]
        .into_iter()
        .collect()
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
    pub async fn add<T: ClusterResource + Sync>(
        &mut self,
        client: &Client,
        resource: T,
    ) -> OperatorResult<T> {
        Self::check_labels(
            resource.labels(),
            &[APP_INSTANCE_LABEL, APP_MANAGED_BY_LABEL, APP_NAME_LABEL],
            &[&self.app_instance, &self.manager, &self.app_name],
        )?;

        if let Some(pod_spec) = resource.pod_spec() {
            pod_spec
                .check_resource_requirement(ResourceRequirementsType::Limits, "cpu")
                .unwrap_or_else(|err| warn!("{}", err));

            pod_spec
                .check_resource_requirement(ResourceRequirementsType::Limits, "memory")
                .unwrap_or_else(|err| warn!("{}", err));

            pod_spec
                .check_limit_to_request_ratio(&ComputeResource::Cpu, LIMIT_REQUEST_RATIO_CPU)
                .unwrap_or_else(|err| warn!("{}", err));

            pod_spec
                .check_limit_to_request_ratio(&ComputeResource::Memory, LIMIT_REQUEST_RATIO_MEMORY)
                .unwrap_or_else(|err| warn!("{}", err));
        }

        let mutated = resource.maybe_mutate(&self.apply_strategy);

        let patched_resource = self
            .apply_strategy
            .run(&self.manager, &mutated, client)
            .await?;

        let resource_id = patched_resource.uid().ok_or(Error::MissingObjectKey {
            key: "metadata/uid",
        })?;

        self.resource_ids.insert(resource_id);

        Ok(patched_resource)
    }

    /// Checks that the given `labels` contain the given `expected_label` with
    /// the given `expected_content`.
    ///
    /// # Arguments
    ///
    /// * `labels` - The labels to check
    /// * `label` - The expected label
    /// * `expected_content` - The expected content of the label
    ///
    /// # Errors
    ///
    /// If `labels` does not contain `label` then an [`Error::MissingLabel`]
    /// is returned.
    ///
    /// If `labels` contains the given `label` but not with the
    /// `expected_content` then an [`Error::UnexpectedLabelContent`]
    /// is returned.
    fn check_label(
        labels: &BTreeMap<String, String>,
        expected_label: &'static str,
        expected_content: &str,
    ) -> OperatorResult<()> {
        if let Some(actual_content) = labels.get(expected_label) {
            if expected_content == actual_content {
                Ok(())
            } else {
                Err(Error::UnexpectedLabelContent {
                    label: expected_label,
                    expected_content: expected_content.into(),
                    actual_content: actual_content.into(),
                })
            }
        } else {
            Err(Error::MissingLabel {
                label: expected_label,
            })
        }
    }

    /// Checks that the given `labels` contain all given `expected_labels` with
    /// the given `expected_contents`.
    fn check_labels(
        labels: &BTreeMap<String, String>,
        expected_labels: &[&'static str],
        expected_contents: &[&str],
    ) -> OperatorResult<()> {
        for (label, content) in expected_labels.iter().zip(expected_contents) {
            Self::check_label(labels, label, content)?;
        }

        Ok(())
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
    ///
    pub async fn delete_orphaned_resources(self, client: &Client) -> OperatorResult<()> {
        tokio::try_join!(
            self.delete_orphaned_resources_of_kind::<Service>(client),
            self.delete_orphaned_resources_of_kind::<StatefulSet>(client),
            self.delete_orphaned_resources_of_kind::<DaemonSet>(client),
            self.delete_orphaned_resources_of_kind::<Job>(client),
            self.delete_orphaned_resources_of_kind::<ConfigMap>(client),
            self.delete_orphaned_resources_of_kind::<Secret>(client),
            self.delete_orphaned_resources_of_kind::<ServiceAccount>(client),
            self.delete_orphaned_resources_of_kind::<RoleBinding>(client),
            self.delete_orphaned_resources_of_kind::<PodDisruptionBudget>(client),
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
        if !self.apply_strategy.delete_orphans() {
            debug!(
                "Skip deleting orphaned resources because of [{}] strategy.",
                self.apply_strategy
            );
            return Ok(());
        }

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
            .list_with_label_selector::<T>(&self.namespace, &label_selector)
            .await?;

        Ok(resources)
    }
}
