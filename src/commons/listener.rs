//! This modules provides resource types used to interact with [listener-operator](https://docs.stackable.tech/listener-operator/stable/index.html)
//!
//! # Custom Resources
//!
//! ## [`Listener`]
//!
//! Exposes a set of pods, either internally to the cluster or to the outside world. The mechanism for how it is exposed
//! is managed by the [`ListenerClass`].
//!
//! It can be either created manually by the application administrator (for applications that expose a single load-balanced endpoint),
//! or automatically when mounting a [listener volume](`ListenerOperatorVolumeSourceBuilder`) (for applications that expose a separate endpoint
//! per replica).
//!
//! All exposed pods *must* have a mounted [listener volume](`ListenerOperatorVolumeSourceBuilder`), regardless of whether the [`Listener`] is created automatically.
//!
//! ## [`ListenerClass`]
//!
//! Declares a policy for how [`Listener`]s are exposed to users.
//!
//! It is created by the cluster administrator.
//!
//! ## [`PodListeners`]
//!
//! Informs users and other operators about the state of all [`Listener`]s associated with a [`Pod`].
//!
//! It is created by the Stackable Secret Operator, and always named `pod-{pod.metadata.uid}`.

use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use k8s_openapi::api::core::v1::{
    Node, PersistentVolume, PersistentVolumeClaim, Pod, Service, Volume,
};

#[cfg(doc)]
use crate::builder::ListenerOperatorVolumeSourceBuilder;

/// Defines a policy for how [`Listener`]s should be exposed.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[kube(
    group = "listeners.stackable.tech",
    version = "v1alpha1",
    kind = "ListenerClass"
)]
#[serde(rename_all = "camelCase")]
pub struct ListenerClassSpec {
    pub service_type: ServiceType,
    /// Annotations that should be added to the [`Service`] object.
    #[serde(default)]
    pub service_annotations: BTreeMap<String, String>,
}

/// The method used to access the services.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
pub enum ServiceType {
    /// Reserve a port on each node.
    NodePort,
    /// Provision a dedicated load balancer.
    LoadBalancer,
    /// Assigns an IP address from a pool of IP addresses that your cluster has reserved for that purpose.
    ClusterIP,
}

/// Exposes a set of pods to the outside world.
///
/// Essentially a Stackable extension of a Kubernetes [`Service`]. Compared to [`Service`], [`Listener`] changes three things:
/// 1. It uses a cluster-level policy object ([`ListenerClass`]) to define how exactly the exposure works
/// 2. It has a consistent API for reading back the exposed address(es) of the service
/// 3. The [`Pod`] must mount a [`Volume`] referring to the `Listener`, which also allows us to control stickiness
#[derive(
    CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, Default, PartialEq, Eq,
)]
#[kube(
    group = "listeners.stackable.tech",
    version = "v1alpha1",
    kind = "Listener",
    namespaced,
    status = "ListenerStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct ListenerSpec {
    /// The name of the [`ListenerClass`].
    pub class_name: Option<String>,
    /// Extra labels that the [`Pod`]s must match in order to be exposed. They must _also_ still have a `Volume` referring to the listener.
    #[serde(default)]
    pub extra_pod_selector_labels: BTreeMap<String, String>,
    /// Ports that should be exposed.
    pub ports: Option<Vec<ListenerPort>>,
    /// Whether incoming traffic should also be directed to `Pod`s that are not `Ready`.
    #[schemars(default = "Self::default_publish_not_ready_addresses")]
    pub publish_not_ready_addresses: Option<bool>,
}

impl ListenerSpec {
    fn default_publish_not_ready_addresses() -> Option<bool> {
        Some(true)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerPort {
    /// The name of the port.
    ///
    /// The name of each port *must* be unique within a single [`Listener`].
    pub name: String,
    /// The port number.
    pub port: i32,
    /// The layer-4 protocol (`TCP` or `UDP`).
    pub protocol: Option<String>,
}

/// Informs users about how to reach the [`Listener`].
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    /// The backing Kubernetes [`Service`].
    pub service_name: Option<String>,
    /// All addresses that the [`Listener`] is currently reachable from.
    pub ingress_addresses: Option<Vec<ListenerIngress>>,
    /// Port mappings for accessing the [`Listener`] on each [`Node`] that the [`Pod`]s are currently running on.
    ///
    /// This is only intended for internal use by listener-operator itself. This will be left unset if using a [`ListenerClass`] that does
    /// not require [`Node`]-local access.
    pub node_ports: Option<BTreeMap<String, i32>>,
}

/// One address that a [`Listener`] is accessible from.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerIngress {
    /// The hostname or IP address to the [`Listener`].
    pub address: String,
    /// The type of address (`Hostname` or `IP`).
    pub address_type: AddressType,
    /// Port mapping table.
    pub ports: BTreeMap<String, i32>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AddressType {
    Hostname,
    #[serde(rename = "IP")]
    Ip,
}

/// Informs users about [`Listener`]s that are bound by a given [`Pod`].
///
/// This is not expected to be created or modified by users. It will be created by
/// the Stackable Listener Operator when mounting the listener volume, and is always
/// named `pod-{pod.metadata.uid}`.
#[derive(
    CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, Default, PartialEq, Eq,
)]
#[kube(
    group = "listeners.stackable.tech",
    version = "v1alpha1",
    kind = "PodListeners",
    namespaced,
    plural = "podlisteners"
)]
#[serde(rename_all = "camelCase")]
pub struct PodListenersSpec {
    /// All listeners currently bound by the [`Pod`].
    ///
    /// Indexed by [`Volume`] name (not [`PersistentVolume`] or [`PersistentVolumeClaim`]).
    pub listeners: BTreeMap<String, PodListener>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PodListener {
    /// `Node` if this address only allows access to [`Pod`]s hosted on a specific Kubernetes [`Node`], otherwise `Cluster`.
    pub scope: PodListenerScope,
    /// Addresses allowing access to this [`Pod`].
    ///
    /// Compared to [`ListenerStatus::ingress_addresses`], this list is restricted to addresses that can access this [`Pod`].
    ///
    /// This field is intended to be equivalent to the files mounted into the listener volume.
    pub ingress_addresses: Option<Vec<ListenerIngress>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PodListenerScope {
    Node,
    Cluster,
}
