use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::crd::listener::AddressType;

/// Exposes a set of pods to the outside world.
///
/// Essentially a Stackable extension of a Kubernetes Service. Compared to a Service, a Listener changes three things:
/// 1. It uses a cluster-level policy object (ListenerClass) to define how exactly the exposure works
/// 2. It has a consistent API for reading back the exposed address(es) of the service
/// 3. The Pod must mount a Volume referring to the Listener, which also allows
///    ["sticky" scheduling](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listener#_sticky_scheduling).
///
/// Learn more in the [Listener documentation](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listener).
#[derive(
    CustomResource, Serialize, Deserialize, Default, Clone, Debug, JsonSchema, PartialEq, Eq,
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
    /// The name of the [ListenerClass](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listenerclass).
    pub class_name: Option<String>,

    /// Extra labels that the Pods must match in order to be exposed. They must _also_ still have a Volume referring to the Listener.
    #[serde(default)]
    pub extra_pod_selector_labels: BTreeMap<String, String>,

    /// Ports that should be exposed.
    pub ports: Option<Vec<ListenerPort>>,

    /// Whether incoming traffic should also be directed to Pods that are not `Ready`.
    #[serde(default = "ListenerSpec::default_publish_not_ready_addresses")]
    pub publish_not_ready_addresses: Option<bool>,
}

impl ListenerSpec {
    const fn default_publish_not_ready_addresses() -> Option<bool> {
        Some(true)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerPort {
    /// The name of the port.
    ///
    /// The name of each port *must* be unique within a single Listener.
    pub name: String,
    /// The port number.
    pub port: i32,
    /// The layer-4 protocol (`TCP` or `UDP`).
    pub protocol: Option<String>,
}

/// Informs users about how to reach the Listener.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    /// The backing Kubernetes Service.
    pub service_name: Option<String>,
    /// All addresses that the Listener is currently reachable from.
    pub ingress_addresses: Option<Vec<ListenerIngress>>,
    /// Port mappings for accessing the Listener on each Node that the Pods are currently running on.
    ///
    /// This is only intended for internal use by listener-operator itself. This will be left unset if using a ListenerClass that does
    /// not require Node-local access.
    pub node_ports: Option<BTreeMap<String, i32>>,
}

/// One address that a Listener is accessible from.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerIngress {
    /// The hostname or IP address to the Listener.
    pub address: String,
    /// The type of address (`Hostname` or `IP`).
    pub address_type: AddressType,
    /// Port mapping table.
    pub ports: BTreeMap<String, i32>,
}

/// Informs users about Listeners that are bound by a given Pod.
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
    /// All Listeners currently bound by the Pod.
    ///
    /// Indexed by Volume name (not PersistentVolume or PersistentVolumeClaim).
    pub listeners: BTreeMap<String, PodListener>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PodListener {
    /// `Node` if this address only allows access to Pods hosted on a specific Kubernetes Node, otherwise `Cluster`.
    pub scope: PodListenerScope,
    /// Addresses allowing access to this Pod.
    ///
    /// Compared to `ingress_addresses` on the Listener status, this list is restricted to addresses that can access this Pod.
    ///
    /// This field is intended to be equivalent to the files mounted into the Listener volume.
    pub ingress_addresses: Option<Vec<ListenerIngress>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PodListenerScope {
    Node,
    Cluster,
}
