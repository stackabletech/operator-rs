use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[cfg(doc)]
use crate::crd::listener::class::v1alpha1::ListenerClass;

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// The method used to access the services.
    //
    // Please note that this does not necessarily need to be restricted to the same Service types
    // Kubernetes supports. Listeners currently happens to support the same set of service types as
    // upstream Kubernetes, but we still want to have the freedom to add custom ones in the future
    // (for example: Istio ingress?).
    #[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
    pub enum ServiceType {
        /// Reserve a port on each node.
        NodePort,

        /// Provision a dedicated load balancer.
        LoadBalancer,

        /// Assigns an IP address from a pool of IP addresses that your cluster has reserved for
        /// that purpose.
        ClusterIP,
    }

    /// Service Internal Traffic Policy enables internal traffic restrictions to only route internal
    /// traffic to endpoints within the node the traffic originated from. The "internal" traffic
    /// here refers to traffic originated from Pods in the current cluster. This can help to reduce
    /// costs and improve performance. See [Kubernetes docs][k8s-docs].
    ///
    /// [k8s-docs]: https://kubernetes.io/docs/concepts/services-networking/service-traffic-policy/
    //
    // Please note that this represents a Kubernetes type, so the name of the enum variant needs to
    // exactly match the Kubernetes traffic policy.
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq, strum::Display)]
    pub enum KubernetesTrafficPolicy {
        /// Obscures the client source IP and may cause a second hop to another node, but allows
        /// Kubernetes to spread the load between all nodes.
        Cluster,

        /// Preserves the client source IP and avoid a second hop for LoadBalancer and NodePort type
        /// Services, but makes clients responsible for spreading the load.
        Local,
    }

    /// The type of a given address.
    #[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
    #[serde(rename_all = "PascalCase")]
    pub enum AddressType {
        /// A resolvable DNS hostname.
        Hostname,

        /// A resolved IP address.
        #[serde(rename = "IP")]
        Ip,
    }

    /// A mode for deciding the preferred [`v1alpha1::AddressType`].
    ///
    /// These can vary depending on the rest of the [`ListenerClass`][lc].
    ///
    /// [lc]: crate::crd::listener::class::v1alpha1::ListenerClass
    #[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, PartialEq, Eq)]
    pub enum PreferredAddressType {
        /// Like [`AddressType::Hostname`], but prefers [`AddressType::Ip`] for [`ServiceType::NodePort`], since their hostnames are less likely to be resolvable.
        HostnameConservative,

        // Like the respective variants of AddressType. Ideally we would refer to them instead of
        // copy/pasting, but that breaks due to upstream issues:
        //
        // - https://github.com/GREsau/schemars/issues/222
        // - https://github.com/kube-rs/kube/issues/1622
        Hostname,

        #[serde(rename = "IP")]
        Ip,
    }
}
