pub mod client;
pub mod controller;
pub mod controller_ref;
pub mod crd;
pub mod error;
pub mod finalizer;
pub mod history;
pub mod k8s_errors;
pub mod metadata;
pub mod podutils;
pub mod reconcile;

use crate::error::OperatorResult;

pub use crd::Crd;
use k8s_openapi::api::core::v1::{ConfigMap, Toleration};
use kube::api::{Meta, ObjectMeta};
use std::collections::BTreeMap;
use tracing_subscriber::EnvFilter;

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
            owner_references: Some(vec![metadata::object_to_owner_reference::<T>(
                resource.meta().clone(),
            )?]),
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
