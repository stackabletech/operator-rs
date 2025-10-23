//! This module contains resource types to interact with [`v1alpha1::ListenerClass`]es.
//!
//! It declares a policy for how [`v1alpha1::Listener`][listener]s are exposed to users. It is
//! created by the cluster administrator.
//!
//! [listener]: crate::crd::listener::listeners::v1alpha1::Listener

use std::collections::BTreeMap;

#[cfg(doc)]
use k8s_openapi::api::core::v1::Service;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

use crate::crd::listener::core::v1alpha1 as core_v1alpha1;
#[cfg(doc)]
use crate::crd::listener::listeners::v1alpha1::Listener;

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// Defines a policy for how [Listeners](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listener) should be exposed.
    /// Read the [ListenerClass documentation](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listenerclass)
    /// for more information.
    #[versioned(crd(group = "listeners.stackable.tech"))]
    #[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct ListenerClassSpec {
        pub service_type: core_v1alpha1::ServiceType,

        /// Configures whether a LoadBalancer service should also allocate node ports (like NodePort).
        ///
        /// Ignored unless serviceType is LoadBalancer.
        // TODO: v1alpha2: Move into ServiceType::LoadBalancer
        #[serde(default = "ListenerClassSpec::default_load_balancer_allocate_node_ports")]
        pub load_balancer_allocate_node_ports: bool,

        /// Configures a custom Service loadBalancerClass, which can be used to access secondary
        /// load balancer controllers that are installed in the cluster, or to provision
        /// custom addresses manually.
        ///
        /// Ignored unless serviceType is LoadBalancer.
        // TODO: v1alpha2: Move into ServiceType::LoadBalancer
        pub load_balancer_class: Option<String>,

        /// Annotations that should be added to the Service object.
        #[serde(default)]
        pub service_annotations: BTreeMap<String, String>,

        /// `externalTrafficPolicy` that should be set on the created Service objects.
        ///
        /// It is a Kubernetes feature that controls how external traffic is routed to a Kubernetes
        /// Service.
        ///
        /// * `Cluster`: Kubernetes default. Traffic is routed to any node in the Kubernetes cluster that
        ///   has a pod running the service.
        /// * `Local`: Traffic is only routed to pods running on the same node as the Service.
        ///
        /// The `Local` mode has better performance as it avoids a network hop, but requires a more
        /// sophisticated LoadBalancer, that respects what Pods run on which nodes and routes traffic only
        /// to these nodes accordingly. Some cloud providers or bare metal installations do not implement
        /// some of the required features.
        //
        // Pls note that we shouldn't mandate the default, but just let Kubernetes choose what to do
        // (currently this means defaulting to Cluster), as this sound the most future-proof to me.
        // Maybe in the future k8s defaults to Local if the LoadBalancer supports it
        pub service_external_traffic_policy: Option<core_v1alpha1::KubernetesTrafficPolicy>,

        /// Whether addresses should prefer using the IP address (`IP`) or the hostname (`Hostname`).
        /// Can also be set to `HostnameConservative`, which will use `IP` for `NodePort` service types, but `Hostname` for everything else.
        ///
        /// The other type will be used if the preferred type is not available.
        ///
        /// Defaults to `HostnameConservative`.
        #[serde(default = "ListenerClassSpec::default_preferred_address_type")]
        pub preferred_address_type: core_v1alpha1::PreferredAddressType,

        /// Whether or not a Pod exposed using a NodePort should be pinned to a specific Kubernetes node.
        ///
        /// By pinning the Pod to a specific (stable) Kubernetes node, stable addresses can be
        /// provided using NodePorts. The pinning is achieved by listener-operator setting the
        /// `volume.kubernetes.io/selected-node` annotation on the Listener PVC.
        ///
        /// However, this only works on setups with long-living nodes. If your nodes are rotated on
        /// a regular basis, the Pods previously running on a removed node will be stuck in Pending
        /// until you delete the PVC with the pinning.
        ///
        /// Because of this we don't enable pinning by default to support all environments.
        #[serde(default)]
        pub pinned_node_ports: bool,
    }
}
