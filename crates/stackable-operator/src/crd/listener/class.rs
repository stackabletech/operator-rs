use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::crd::listener::{
    AddressType, KubernetesTrafficPolicy, PreferredAddressType, ServiceType,
};

/// Defines a policy for how [Listeners](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listener) should be exposed.
/// Read the [ListenerClass documentation](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listenerclass)
/// for more information.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
#[kube(
    group = "listeners.stackable.tech",
    version = "v1alpha1",
    kind = "ListenerClass"
)]
#[serde(rename_all = "camelCase")]
pub struct ListenerClassSpec {
    pub service_type: ServiceType,

    /// Annotations that should be added to the Service object.
    #[serde(default)]
    pub service_annotations: BTreeMap<String, String>,

    /// `externalTrafficPolicy` that should be set on the created [`Service`] objects.
    ///
    /// The default is `Local` (in contrast to `Cluster`), as we aim to direct traffic to a node running the workload
    /// and we should keep testing that as the primary configuration. Cluster is a fallback option for providers that
    /// break Local mode (IONOS so far).
    #[serde(default = "ListenerClassSpec::default_service_external_traffic_policy")]
    pub service_external_traffic_policy: KubernetesTrafficPolicy,

    /// Whether addresses should prefer using the IP address (`IP`) or the hostname (`Hostname`).
    /// Can also be set to `HostnameConservative`, which will use `IP` for `NodePort` service types, but `Hostname` for everything else.
    ///
    /// The other type will be used if the preferred type is not available.
    ///
    /// Defaults to `HostnameConservative`.
    #[serde(default = "ListenerClassSpec::default_preferred_address_type")]
    pub preferred_address_type: PreferredAddressType,
}

impl ListenerClassSpec {
    const fn default_service_external_traffic_policy() -> KubernetesTrafficPolicy {
        KubernetesTrafficPolicy::Local
    }

    const fn default_preferred_address_type() -> PreferredAddressType {
        PreferredAddressType::HostnameConservative
    }

    /// Resolves [`Self::preferred_address_type`]'s "smart" modes depending on the rest of `self`.
    pub fn resolve_preferred_address_type(&self) -> AddressType {
        self.preferred_address_type.resolve(self)
    }
}
