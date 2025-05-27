use crate::crd::listener::{
    class::v1alpha1::ListenerClassSpec,
    core::v1alpha1::{AddressType, KubernetesTrafficPolicy, PreferredAddressType},
};

impl ListenerClassSpec {
    pub(super) const fn default_service_external_traffic_policy() -> KubernetesTrafficPolicy {
        KubernetesTrafficPolicy::Local
    }

    pub(super) const fn default_preferred_address_type() -> PreferredAddressType {
        PreferredAddressType::HostnameConservative
    }

    pub(super) const fn default_load_balancer_allocate_node_ports() -> bool {
        true
    }

    /// Resolves [`Self::preferred_address_type`]'s "smart" modes depending on the rest of `self`.
    pub fn resolve_preferred_address_type(&self) -> AddressType {
        self.preferred_address_type.resolve(self)
    }
}
