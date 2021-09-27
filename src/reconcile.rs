use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::k8s_utils::LabelOptionalValueMap;
use crate::{conditions, controller_ref, finalizer, pod_utils};

use crate::command::{
    clear_current_command, maybe_update_current_command, CanBeRolling, CommandRef, HasCommands,
    HasRoleRestartOrder, HasRoles,
};
use crate::command_controller::Command;
use crate::conditions::ConditionStatus;
use crate::crd::HasApplication;
use crate::error::Error::{InvalidName, KubeError};
use crate::k8s_utils::find_excess_pods;
use crate::labels::{
    APP_COMPONENT_LABEL, APP_INSTANCE_LABEL, APP_NAME_LABEL, APP_ROLE_GROUP_LABEL,
};
use crate::status::HasCurrentCommand;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::{ConfigMap, Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    Condition, LabelSelector, LabelSelectorRequirement,
};
use kube::api::{ObjectMeta, ResourceExt};
use kube::core::object::HasStatus;
use kube::error::ErrorResponse;
use kube::{CustomResourceExt, Resource};
use kube_runtime::controller::ReconcilerAction;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, info, trace, warn};

pub type ReconcileResult<E> = std::result::Result<ReconcileFunctionAction, E>;

/// Creates a [`ReconcilerAction`] that will trigger a requeue after a specified [`Duration`].
pub fn create_requeuing_reconciler_action(duration: Duration) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(duration),
    }
}

/// Creates a [`ReconcilerAction`] that won't trigger a requeue.
pub fn create_non_requeuing_reconciler_action() -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: None,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReconcileFunctionAction {
    /// Run the next function in the reconciler chain
    Continue,

    /// Skip the remaining reconciler chain
    Done,

    /// Skip the remaining reconciler chain and queue this object again
    Requeue(Duration),
}

impl ReconcileFunctionAction {
    /// Can be used to chain multiple functions which all return a Result<ReconcileFunctionAction, E>.
    ///
    /// Will call the `next` function in the chain only if the previous returned `Continue`.
    /// Will return the result from the last one otherwise.
    pub async fn then<E>(
        self,
        next: impl Future<Output = Result<ReconcileFunctionAction, E>>,
    ) -> Result<ReconcileFunctionAction, E> {
        match self {
            ReconcileFunctionAction::Continue => next.await,
            action => Ok(action),
        }
    }
}

#[derive(Eq, PartialEq)]
pub enum ContinuationStrategy {
    /// Will process all resources (including potential changes) and then continue with the reconciliation
    AllContinue,

    /// Will process all resources (including potential changes) and then requeue the resource
    AllRequeue,

    /// Will process all resources but will return a requeue after any changes
    OneRequeue,
}

pub struct PodLocation {
    role: String,
    role_group: String,
    node: String,
}

#[async_trait]
pub trait ProvidesPod {
    async fn get_pod_and_context(
        &self,
        role: &str,
        role_group: &str,
        node: &str,
    ) -> OperatorResult<(Pod, Vec<ConfigMap>)>; // We should convert this to something like (Vec<Pod>, Vec<DynamicObject>) for more flexibility
}

pub struct ReconciliationContext<T> {
    pub client: Client,
    pub resource: T,
    pub requeue_timeout: Duration,
}

impl<T> ReconciliationContext<T> {
    pub fn new(client: Client, resource: T, requeue_timeout: Duration) -> Self {
        ReconciliationContext {
            client,
            resource,
            requeue_timeout,
        }
    }

    fn requeue(&self) -> ReconcileFunctionAction {
        ReconcileFunctionAction::Requeue(self.requeue_timeout)
    }

    /// This is a reconciliation gate to wait for a list of Pods to be running and ready.
    ///
    /// See [`pod_utils::is_pod_running_and_ready`] for details.
    /// Will requeue as soon as a single Pod is not running or not ready.
    pub async fn wait_for_running_and_ready_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        wait_for_running_and_ready_pods(&self.requeue_timeout, pods)
    }

    /// This is a reconciliation gate to wait for a list of Pods to terminate.
    ///
    /// Will requeue as soon as a single Pod is in the process of terminating.
    pub async fn wait_for_terminating_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        wait_for_terminating_pods(&self.requeue_timeout, pods)
    }
}

impl<T> ReconciliationContext<T>
where
    T: Resource,
{
    pub fn name(&self) -> String {
        self.resource.name()
    }

    pub fn namespace(&self) -> String {
        self.resource.namespace().expect("Resources are namespaced")
    }

    /// Returns a name that is suitable for directly passing to a log macro.
    ///
    /// See [`crate::pod_utils::get_log_name()`] for details.
    pub fn log_name(&self) -> String {
        pod_utils::get_log_name(&self.resource)
    }

    pub fn metadata(&self) -> ObjectMeta {
        self.resource.meta().clone()
    }

    /// This lists all Resources that have an OwnerReference that points to us (the object from `self.resource`)
    /// as its Controller.
    ///
    /// Unfortunately the Kubernetes API does _not_ allow filtering by OwnerReference so we have to fetch
    /// all Resources and filter them on the client.
    /// To reduce this overhead a LabelSelector will be included that uses the standard
    /// `app.kubernetes.io/instance` label and will use the name of the resource in this context
    /// as its value.
    /// You need to make sure to always set this label correctly!
    /// One way to achieve this is by using the [`crate::labels::get_recommended_labels`] method.
    pub async fn list_owned<R>(
        &self,
        match_labels: BTreeMap<String, String>,
    ) -> OperatorResult<Vec<R>>
    where
        R: Clone + Debug + DeserializeOwned + Resource,
        <R as Resource>::DynamicType: Default,
    {
        let owner_uid = self
            .resource
            .meta()
            .uid
            .as_ref()
            .ok_or(Error::MissingObjectKey {
                key: ".metadata.uid",
            })?;

        let label_selector = LabelSelector {
            match_labels: Some(match_labels),
            ..LabelSelector::default()
        };

        self.client
            .list_with_label_selector(self.resource.namespace().as_deref(), &label_selector)
            .await
            .map(|resources| {
                resources
                    .into_iter()
                    .filter(|resource| controller_ref::is_resource_owned_by(resource, owner_uid))
                    .collect()
            })
    }

    /// Creates a new [`Condition`] for the `resource` this context contains.
    ///
    /// It's a convenience function that passes through all parameters and builds a `Condition`
    /// using the [`conditions::build_condition`] method.
    pub fn build_condition_for_resource(
        &self,
        current_conditions: Option<&[Condition]>,
        message: String,
        reason: String,
        status: ConditionStatus,
        condition_type: String,
    ) -> Condition {
        conditions::build_condition(
            &self.resource,
            current_conditions,
            message,
            reason,
            status,
            condition_type,
        )
    }

    /// Checks all passed Pods to see if they fulfil some basic requirements.
    ///
    /// * They need to have all required labels and optionally one of a list of allowed values
    /// * They need to have a spec.node_name
    ///
    /// If not they are considered invalid and will be deleted.
    ///
    /// This is a safety measure and should never actually delete any Pods as all Pods operators create
    /// should obviously all be valid.
    /// If this ever deletes a Pod it'll be either a programming error or a user who created or changed
    /// Pods manually.
    ///
    /// Implementation note: Unfortunately the required label structure is slightly different here than in `delete_excess_pods`
    /// and while that one could be converted into the one we need it'd require another parameter
    /// to ignore certain labels (e.g. `role group` values should never be checked)
    pub async fn delete_illegal_pods(
        &self,
        pods: &[Pod],
        required_labels: &BTreeMap<String, Option<Vec<String>>>,
        deletion_strategy: ContinuationStrategy,
    ) -> ReconcileResult<Error> {
        let illegal_pods = pod_utils::find_invalid_pods(pods, required_labels);
        if illegal_pods.is_empty() {
            return Ok(ReconcileFunctionAction::Continue);
        }

        for illegal_pod in illegal_pods {
            warn!(
                "Deleting invalid Pod [{}]",
                pod_utils::get_log_name(illegal_pod)
            );
            self.client.delete(illegal_pod).await?;

            if deletion_strategy == ContinuationStrategy::OneRequeue {
                trace!(
                    "Will requeue after deleting an illegal pod, there might be more illegal ones"
                );
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == ContinuationStrategy::AllRequeue {
            Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }

    /// This method can be used to find Pods that do not match a set of Nodes and required labels.
    ///
    /// All Pods must match at least one of the node list & required labels combinations.
    /// All that don't match will be returned.
    ///
    /// The idea is that you pass in a list of tuples, one tuple for each role group.
    /// Each tuple consists of a list of eligible nodes for that role group's LabelSelector and a
    /// Map of label keys to optional values.
    ///
    /// To clearly identify Pods (e.g. to distinguish two pods on the same node from each other) they
    /// usually need some labels (e.g. a `component` and a `role-group` label).     
    pub async fn delete_excess_pods(
        &self,
        nodes_and_labels: &[(Vec<Node>, LabelOptionalValueMap, Option<u16>)],
        existing_pods: &[Pod],
        deletion_strategy: ContinuationStrategy,
    ) -> ReconcileResult<Error> {
        let excess_pods = find_excess_pods(nodes_and_labels, existing_pods);
        for excess_pod in excess_pods {
            info!(
                "Deleting excess Pod [{}]",
                pod_utils::get_log_name(excess_pod)
            );
            self.client.delete(excess_pod).await?;

            if deletion_strategy == ContinuationStrategy::OneRequeue {
                trace!(
                    "Will requeue after deleting an excess pod, there might be more illegal ones"
                );
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == ContinuationStrategy::AllRequeue {
            Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }

    pub async fn create_missing_pods(&self, creation_strategy: ContinuationStrategy)
    where
        T: ProvidesPod + HasRoleRestartOrder,
    {
    }

    /// This method can be used to ensure a ConfigMap exists and has the specified content.
    ///
    /// If a ConfigMap with the specified name does not exist it will be created.
    ///
    /// Should a ConfigMap with the specified name already exist the content is retrieved and
    /// compared with the content from `config_map`, if content differs the existing ConfigMap is
    /// updated.
    ///
    /// Returns `Ok(true)` if a change was made and `Ok(false}` if no change was necessary.
    pub async fn create_config_map(&self, config_map: ConfigMap) -> OperatorResult<bool> {
        let cm_name = match config_map.metadata.name.as_deref() {
            None => {
                return Err(InvalidName {
                    errors: vec![String::from(
                        "ConfigMap with empty name encountered, this is illegal!",
                    )],
                })
            }
            Some(name) => name,
        };

        match self
            .client
            .get::<ConfigMap>(cm_name, Some(&self.namespace()))
            .await
        {
            Ok(ConfigMap {
                data: existing_config_map_data,
                ..
            }) if existing_config_map_data == config_map.data => {
                info!(
                    "ConfigMap [{}] already exists with identical data, skipping creation!",
                    cm_name
                );
                Ok(false)
            }
            Ok(_) => {
                info!(
                    "ConfigMap [{}] already exists, but differs, updating it!",
                    cm_name
                );
                self.client.update(&config_map).await?;
                Ok(true)
            }
            Err(KubeError {
                source: kube::error::Error::Api(ErrorResponse { reason, .. }),
            }) if reason == "NotFound" => {
                info!("ConfigMap [{}] not found, creating it.", cm_name);
                self.client.create(&config_map).await?;
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn default_restart<C, P>(
        &self,
        command: &C,
        pod_provider: &P,
    ) -> ReconcileResult<Error>
    where
        T: HasApplication + HasRoleRestartOrder,
        C: Command + CanBeRolling + HasRoles,
        // TODO: Not sure if we can skip 'HasRoles' here and conditionally run code below if it is implemented
        P: ProvidesPod,
    {
        // If the command provides a list of roles this overrides the default provided by the cluster
        // definition itself
        let role_order = command
            .get_role_order()
            .unwrap_or_else(T::get_role_restart_order);

        let mut restart_occurred = false;
        for role in role_order {
            // Retrieve all pods for this service and role
            let selector = self.get_role_selector(&role);
            let pods = self
                .client
                .list_with_label_selector::<Pod>(self.resource.namespace().as_deref(), &selector)
                .await?;

            // Filter those out that have been restarted since the command was started
            let pods = pods
                .iter()
                .filter(
                    |pod| match (&pod.metadata.creation_timestamp, &command.start_time()) {
                        (Some(pod_start_time), Some(command_start_time)) => {
                            warn!(
                                "Comparing times: [{}] < [{}]",
                                pod_start_time.0, command_start_time
                            );
                            &pod_start_time.0 < command_start_time
                                && pod.metadata.deletion_timestamp.is_none()
                        }
                        _ => {
                            warn!("One of the times was not set!");
                            false
                        }
                    },
                )
                .collect::<Vec<_>>();

            if pods.is_empty() {
                // Got no pods for this role, skip rest of processing
                warn!(
                    "Skipping role [{}] during restart, no pods left to restart.",
                    role
                );
                continue;
            }

            // Track if anything was changed during this run
            restart_occurred = true;
            // Restart pods depending on strategy
            match command.is_rolling() {
                true => {
                    let current_pod = pods.first().unwrap().deref();
                    self.client.ensure_deleted(current_pod.clone()).await?;
                    return Ok(ReconcileFunctionAction::Requeue(Duration::from_secs(5)));

                    //if let Some(labels) = &current_pod.meta().labels {
                    // let role_group = labels.get(APP_ROLE_GROUP_LABEL).unwrap();
                    // let node = current_pod
                    //     .spec
                    //     .clone()
                    //     .unwrap_or_default()
                    //     .node_name
                    //     .unwrap();

                    //self.client.ensure_deleted(current_pod.clone()).await?;

                    // let (pods, _) = pod_provider
                    //     .get_pod_and_context(&role, role_group, &node)
                    //     .await?;
                    //
                    // //for config_map in config_maps {
                    // //    self.create_config_map(config_map).await?;
                    // // }
                    // warn!("Creating pod on node [{}]: [{:?}]", node, pods);
                    // self.client.create(&pods).await?;
                    // // We return early for this case, there is nothing left to do after one pod
                    // // was restarted
                    //return Ok(ReconcileFunctionAction::Requeue(Duration::from_secs(5)));
                    //}
                }
                false => {
                    for pod in pods {
                        self.client.delete(pod).await?;
                    }
                }
            }
        }
        if restart_occurred {
            Ok(ReconcileFunctionAction::Requeue(Duration::from_secs(5)))
        } else {
            // No pods that were eligible for a restart were found, restart is done
            Ok(ReconcileFunctionAction::Done)
        }
    }

    fn get_role_selector(&self, role: &str) -> LabelSelector
    where
        T: HasApplication,
    {
        let application_match = LabelSelectorRequirement {
            key: APP_NAME_LABEL.to_string(),
            operator: "In".to_string(),
            values: Some(vec![
                <T as HasApplication>::get_application_name().to_string()
            ]),
        };
        let role_match = LabelSelectorRequirement {
            key: APP_COMPONENT_LABEL.to_string(),
            operator: "In".to_string(),
            values: Some(vec![role.to_string()]),
        };

        let instance_match = LabelSelectorRequirement {
            key: APP_INSTANCE_LABEL.to_string(),
            operator: "In".to_string(),
            values: Some(vec![self.resource.name()]),
        };

        let result = LabelSelector {
            match_expressions: Some(vec![application_match, role_match, instance_match]),
            match_labels: Default::default(),
        };
        warn!("Created labelselector: [{:?}]", result);
        result
    }

    pub async fn retrieve_current_command(&mut self) -> OperatorResult<Option<CommandRef>>
    where
        T: Clone
            + Debug
            + DeserializeOwned
            + Resource
            + CustomResourceExt
            + HasCommands
            + HasStatus
            + Send
            + Sync
            + 'static,
        <T as Resource>::DynamicType: Default,
        <T as HasStatus>::Status: HasCurrentCommand + Debug + Default + Serialize,
    {
        let current_command_ref = crate::command::current_command(
            &self.resource,
            T::get_command_types().as_slice(),
            &self.client,
        )
        .await?;

        // Check if the one that should be running has already been set in the Status => was
        // already started
        Ok(match current_command_ref {
            None => None,
            Some(current_command) => {
                maybe_update_current_command(&mut self.resource, &current_command, &self.client)
                    .await?;
                Some(current_command)
            }
        })
    }

    /// This reconcile function can be added to the chain to automatically handle deleted objects
    /// using finalizers.
    ///
    /// It'll add a finalizer to the object if it's not there yet, if the `deletion_timestamp` is set
    /// it'll call the provided handler function and it'll remove the finalizer if the handler completes
    /// with a `Done` result.
    ///
    /// If the object is not deleted this function will return a `Continue` event.
    ///
    /// # Arguments
    ///
    /// * `handler` - This future will be completed if the object has been marked for deletion
    /// * `finalizer` - The finalizer to add and/or check for
    /// * `requeue_if_changed` - If this is `true` we'll return a `Requeue` immediately if we had to
    ///     change the resource due to the addition of the finalizer
    pub async fn handle_deletion(
        &self,
        handler: Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Error>> + Send + '_>>,
        finalizer: &str,
        requeue_if_changed: bool,
    ) -> ReconcileResult<Error>
    where
        T: Clone + Debug + DeserializeOwned + Resource + Send + Sync + 'static,
        <T as Resource>::DynamicType: Default,
    {
        let being_deleted = finalizer::has_deletion_stamp(&self.resource);

        // Try to add a finalizer but only if the deletion_timestamp is not already set
        // Kubernetes forbids setting new finalizers on objects under deletion and will return this error:
        // Forbidden: no new finalizers can be added if the object is being deleted, found new finalizers []string{\"foo\"}
        if !being_deleted
            && finalizer::add_finalizer(&self.client, &self.resource, finalizer).await?
            && requeue_if_changed
        {
            return Ok(self.requeue());
        }

        if !being_deleted {
            debug!("Resource not deleted, continuing",);
            return Ok(ReconcileFunctionAction::Continue);
        }

        if !finalizer::has_finalizer(&self.resource, finalizer) {
            debug!("Resource being deleted but our finalizer is already gone, there might be others but we're done here!");
            return Ok(ReconcileFunctionAction::Done);
        }

        match handler.await? {
            ReconcileFunctionAction::Continue => Ok(ReconcileFunctionAction::Continue),
            ReconcileFunctionAction::Done => {
                info!("Removing finalizer [{}]", finalizer,);
                finalizer::remove_finalizer(&self.client, &self.resource, finalizer).await?;
                Ok(ReconcileFunctionAction::Done)
            }
            ReconcileFunctionAction::Requeue(_) => Ok(self.requeue()),
        }
    }
}

impl<T> ReconciliationContext<T>
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
{
    /// Sets the [`Condition`] on the resource in this context.
    pub async fn set_condition(&self, condition: Condition) -> OperatorResult<T> {
        Ok(self.client.set_condition(&self.resource, condition).await?)
    }

    /// Builds a [`Condition`] using [`ReconciliationContext::build_condition_for_resource`] and then sets saves it.
    pub async fn build_and_set_condition(
        &self,
        current_conditions: Option<&[Condition]>,
        message: String,
        reason: String,
        status: ConditionStatus,
        condition_type: String,
    ) -> OperatorResult<T> {
        let condition = self.build_condition_for_resource(
            current_conditions,
            message,
            reason,
            status,
            condition_type,
        );
        self.set_condition(condition).await
    }

    /// A reconciler function to add to our finalizer to the list of finalizers.
    /// It is a wrapper around [`finalizer::add_finalizer`].
    ///
    /// It can return `Continue` or `Requeue` depending on the `requeue` argument and the state of the resource.
    /// If the finalizer already exists it'll _always_ return `Continue`.
    ///
    /// There is a more full-featured alternative to this function ([`Self::handle_deletion`]).
    ///
    /// # Arguments
    ///
    /// * `finalizer` - The finalizer to add
    /// * `requeue` - If `true` this function will return `Requeue` if the object was changed (i.e. the finalizer was added) otherwise it'll return `Continue`
    pub async fn add_finalizer(&self, finalizer: &str, requeue: bool) -> ReconcileResult<Error> {
        if finalizer::add_finalizer(&self.client, &self.resource, finalizer).await? && requeue {
            Ok(self.requeue())
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }
}

fn wait_for_running_and_ready_pods(
    requeue_timeout: &Duration,
    pods: &[Pod],
) -> ReconcileResult<Error> {
    let not_ready = pods
        .iter()
        .filter(|pod| !pod_utils::is_pod_running_and_ready(pod))
        .collect::<Vec<_>>();

    if !not_ready.is_empty() {
        let pods = not_ready
            .iter()
            .map(|pod| pod_utils::get_log_name(*pod))
            .collect::<Vec<_>>();
        let pods = pods.join(", ");
        trace!("Waiting for Pods to become ready: [{}]", pods);
        return Ok(ReconcileFunctionAction::Requeue(*requeue_timeout));
    }

    Ok(ReconcileFunctionAction::Continue)
}

fn wait_for_terminating_pods(requeue_timeout: &Duration, pods: &[Pod]) -> ReconcileResult<Error> {
    match pods.iter().any(|pod| finalizer::has_deletion_stamp(pod)) {
        true => {
            info!("Found terminating pods, requeuing to await termination!");
            Ok(ReconcileFunctionAction::Requeue(*requeue_timeout))
        }
        false => {
            debug!("No terminating pods found, continuing");
            Ok(ReconcileFunctionAction::Continue)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ObjectMetaBuilder, PodBuilder};
    use chrono::Utc;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

    #[test]
    fn test_wait_for_running_and_ready_pods() {
        let duration = Duration::from_secs(30);
        let action = ReconcileFunctionAction::Requeue(duration);

        let pod1 = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("pod1").build().unwrap())
            .build()
            .unwrap();
        let pod2 = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("pod2").build().unwrap())
            .build()
            .unwrap();
        let pods = vec![pod1, pod2];
        let result = wait_for_running_and_ready_pods(&duration, &pods).unwrap();
        assert_eq!(result, action);

        let result = wait_for_running_and_ready_pods(&duration, &[]).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let pod1 = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("pod1").build().unwrap())
            .phase("Running")
            .build()
            .unwrap();
        let result = wait_for_running_and_ready_pods(&duration, vec![pod1].as_slice()).unwrap();
        assert_eq!(result, action);

        let pod1 = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("pod1").build().unwrap())
            .phase("Running")
            .with_condition("Ready", "True")
            .build()
            .unwrap();
        let result =
            wait_for_running_and_ready_pods(&duration, vec![pod1.clone()].as_slice()).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let pod2 = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("pod2").build().unwrap())
            .build()
            .unwrap();
        let result =
            wait_for_running_and_ready_pods(&duration, vec![pod1, pod2].as_slice()).unwrap();
        assert_eq!(result, action);
    }

    #[test]
    fn test_wait_for_terminating_pods() {
        let duration = Duration::from_secs(30);
        let action = ReconcileFunctionAction::Requeue(duration);

        let mut pod1 = PodBuilder::new().metadata_default().build().unwrap();

        pod1.metadata.deletion_timestamp = Some(Time(Utc::now()));

        let result = wait_for_terminating_pods(&duration, vec![pod1.clone()].as_slice()).unwrap();
        assert_eq!(result, action);

        let pod2 = PodBuilder::new().metadata_default().build().unwrap();
        let result = wait_for_terminating_pods(&duration, vec![pod2.clone()].as_slice()).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let result = wait_for_terminating_pods(&duration, vec![pod1, pod2].as_slice()).unwrap();
        assert_eq!(result, action);
    }
}
