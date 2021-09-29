//! This module provides structs and trades to generalize the custom resource status access.
use crate::client::Client;
use crate::command::CommandRef;
use crate::error::OperatorResult;
use crate::versioning::ProductVersion;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

/// Provides access to the custom resource status.
pub trait Status<T> {
    fn status(&self) -> &Option<T>;
    fn status_mut(&mut self) -> &mut Option<T>;
}

/// Provides access to the custom resource status conditions.
pub trait Conditions {
    fn conditions(&self) -> &[Condition];
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}

pub trait HasCurrentCommand {
    fn current_command(&self) -> Option<CommandRef>;
    // TODO: setters are non-rusty, is there a better way? Dirkjan?
    fn set_current_command(&mut self, command: CommandRef);
    fn tracking_location() -> &'static str;
    fn clear_current_command(&mut self);
}

/// Provides access to the custom resource status version for up or downgrades.
pub trait Versioned<V> {
    fn version(&self) -> &Option<ProductVersion<V>>;
    fn version_mut(&mut self) -> &mut Option<ProductVersion<V>>;
}

pub trait HasClusterExecutionStatus {
    fn cluster_execution_status(&self) -> Option<ClusterExecutionStatus>;
    fn cluster_execution_status_patch(&self, execution_status: &ClusterExecutionStatus) -> Value;
}

/// Signals the current status of the cluster
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum ClusterExecutionStatus {
    Stopped,
    Running,
}

/// Initializes the custom resource status with its default. The status is written to the api
/// server and internally updated for later use. This should be called before anything status
/// related will be processed.
///
/// Returns the updated custom resource for further usage.
///
/// # Arguments
///
/// * `client` - The Kubernetes client.
/// * `resource` - The cluster custom resource.
///
pub async fn init_status<T, S>(client: &Client, resource: &T) -> OperatorResult<T>
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Status<S>,
    S: Debug + Default + Serialize,
{
    if resource.status().is_none() {
        return client.merge_patch_status(resource, &S::default()).await;
    }

    Ok(resource.clone())
}
