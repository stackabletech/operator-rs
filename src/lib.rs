pub mod client;
pub mod controller_ref;
pub mod crd;
pub mod error;
pub mod finalizer;
pub mod history;
pub mod k8s_errors;
pub mod podutils;

use crate::client::Client;
use crate::error::{Error, OperatorResult};
pub use crd::CRD;
use k8s_openapi::api::core::v1::{ConfigMap, Toleration};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use k8s_openapi::Resource;
use kube::api::{Meta, ObjectMeta, PatchParams, PatchStrategy};
use kube::Api;
use kube_runtime::controller::{Context, ReconcilerAction};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use tracing::error;
use tracing_subscriber::EnvFilter;

/// Context data inserted into the reconciliation handler with each call.
pub struct ContextData {
    /// Kubernetes client to manipulate Kubernetes resources
    #[allow(dead_code)]
    pub client: Client,
}

impl ContextData {
    /// Creates a new instance of `ContextData`.
    ///
    /// # Arguments
    ///
    /// - `client` - Kubernetes client to manipulate Kubernetes resources
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }

    pub fn new_context(client: Client) -> Context<ContextData> {
        Context::new(ContextData::new(client))
    }
}

/// Action to be taken by the controller if there is a new event on one of the watched resources
pub enum ControllerAction {
    /// A resource was created
    Create,

    /// A resource was updated
    Update,

    /// The resource is about to be deleted
    Delete,
}

/// Examines the incoming resource and determines the `ControllerAction` to be taken upon it.
pub fn decide_controller_action<T>(resource: &T, finalizer: &str) -> Option<ControllerAction>
where
    T: Meta,
{
    let has_finalizer: bool = finalizer::has_finalizer(resource, finalizer);
    let has_deletion_timestamp: bool = finalizer::has_deletion_stamp(resource);
    if has_finalizer && has_deletion_timestamp {
        Some(ControllerAction::Delete)
    } else if !has_finalizer && !has_deletion_timestamp {
        Some(ControllerAction::Create)
    } else if has_finalizer && !has_deletion_timestamp {
        Some(ControllerAction::Update)
    } else {
        // The object is being deleted but we've already finished our finalizer
        // So there's nothing left to do for us on this one.
        None
    }
}

/// Creates a vector of tolerations we need to work with the Krustlet
pub fn create_tolerations() -> Vec<Toleration> {
    vec![
        Toleration {
            effect: Some(String::from("NoExecute")),
            key: Some(String::from("kubernetes.io/arch")),
            operator: Some(String::from("Equal")),
            toleration_seconds: None,
            value: Some(String::from("stackable-linux")),
        },
        Toleration {
            effect: Some(String::from("NoSchedule")),
            key: Some(String::from("kubernetes.io/arch")),
            operator: Some(String::from("Equal")),
            toleration_seconds: None,
            value: Some(String::from("stackable-linux")),
        },
        Toleration {
            effect: Some(String::from("NoSchedule")),
            key: Some(String::from("node.kubernetes.io/network-unavailable")),
            operator: Some(String::from("Exists")),
            toleration_seconds: None,
            value: None,
        },
    ]
}

pub fn object_to_owner_reference<K: Resource>(meta: ObjectMeta) -> OperatorResult<OwnerReference> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        name: meta.name.ok_or(Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.ok_or(Error::MissingObjectKey {
            key: ".metadata.backtrace",
        })?,
        ..OwnerReference::default()
    })
}

pub async fn patch_resource<T>(
    api: &Api<T>,
    resource_name: &str,
    resource: &T,
    field_manager: &str,
) -> OperatorResult<T>
where
    T: Clone + Meta + DeserializeOwned + Serialize,
{
    api.patch(
        &resource_name,
        &PatchParams {
            patch_strategy: PatchStrategy::Apply,
            field_manager: Some(field_manager.to_string()),
            ..PatchParams::default()
        },
        serde_json::to_vec(&resource)?,
    )
    .await
    .map_err(Error::from)
}

/// Creates a ConfigMap
pub fn create_config_map<T>(
    resource: &T,
    cm_name: &str,
    data: BTreeMap<String, String>,
) -> OperatorResult<ConfigMap>
where
    T: Meta,
{
    let cm = ConfigMap {
        data: Some(data),
        metadata: ObjectMeta {
            name: Some(String::from(cm_name)),
            namespace: Meta::namespace(resource),
            owner_references: Some(vec![OwnerReference {
                controller: Some(true),
                ..object_to_owner_reference::<T>(resource.meta().clone())?
            }]),
            ..ObjectMeta::default()
        },
        ..ConfigMap::default()
    };
    Ok(cm)
}

/// Initializes `tracing` logging with options from the environment variable
/// given in the `env` parameter.
///
/// We force users to provide a variable name so it can be different per product.
/// We encourage it to be the product name plus `_LOG`, e.g. `ZOOKEEPER_OPERATOR_LOG`.
pub fn initialize_logging(env: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env(env))
        .init();
}

pub async fn create_client(field_manager: Option<String>) -> OperatorResult<client::Client> {
    Ok(client::Client::new(
        kube::Client::try_default().await?,
        field_manager,
    ))
}

/// This method returns a closure which can be used as an `error_policy` by the [Controller](kube_runtime::Controller).
/// The returned method will be called whenever there's an error during reconciliation.
/// It just logs the error and requeues the event after a configurable amount of time
///
/// # Example
/// ```ignore
/// use std::time::Duration;
/// use stackable_operator::requeueing_error_policy;
///
/// let error_policy = requeueing_error_policy(Duration::from_secs(10));
/// ```
pub fn requeueing_error_policy<E, T: Sized>(
    duration: Duration,
) -> impl FnMut(&E, Context<T>) -> ReconcilerAction
where
    E: std::fmt::Display,
{
    move |error, _context| {
        error!("Reconciliation error:\n{}", error);
        ReconcilerAction {
            requeue_after: Some(duration),
        }
    }
}
