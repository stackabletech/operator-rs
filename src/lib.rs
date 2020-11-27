pub mod crd;
pub mod error;
pub mod finalizer;

pub use crd::CRD as CRD;
use kube::{Client, Api};
use kube_runtime::controller::Context;
use kube::api::{Meta, ObjectMeta, PatchParams, PatchStrategy};
use k8s_openapi::api::core::v1::{Toleration, ConfigMap};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use crate::error::Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;

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
    where T: Meta
{
    let has_finalizer: bool = finalizer::has_finalizer(resource, finalizer);
    let has_deletion_timestamp: bool = finalizer::has_deletion_stamp(resource);
    return if has_finalizer && has_deletion_timestamp {
        Some(ControllerAction::Delete)
    } else if !has_finalizer && !has_deletion_timestamp {
        Some(ControllerAction::Create)
    } else if has_finalizer && !has_deletion_timestamp {
        Some(ControllerAction::Update)
    } else {
        // The object is being deleted but we've already finished our finalizer
        // So there's nothing left to do for us on this one.
        None
    };
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

pub fn object_to_owner_reference<K: Meta>(meta: ObjectMeta) -> Result<OwnerReference, Error> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        name: meta.name.ok_or_else(|| Error::MissingObjectKey {
            key: ".metadata.name",
        })?,
        uid: meta.uid.ok_or_else(|| Error::MissingObjectKey {
            key: ".metadata.backtrace",
        })?,
        ..OwnerReference::default()
    })
}

pub async fn patch_resource<T>(api: &Api<T>, resource_name: &String, resource: &T, field_manager: &str) -> Result<T, Error>
    where T: Clone + Meta + DeserializeOwned + Serialize,
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
    cm_name: &String,
    data: BTreeMap<String, String>,
) -> Result<ConfigMap, Error>
    where T: Meta
{
    let cm = ConfigMap {
        data: Some(data),
        metadata: ObjectMeta {
            name: Some(cm_name.clone()),
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

pub fn initialize_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
}

pub async fn create_client() -> Result<kube::Client, error::Error> {
    return Ok(kube::Client::try_default().await?);
}
