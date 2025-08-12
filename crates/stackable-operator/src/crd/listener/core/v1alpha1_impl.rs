use crate::crd::listener::{
    class::v1alpha1::ListenerClassSpec,
    core::v1alpha1::{AddressType, PreferredAddressType, ServiceType},
};

impl PreferredAddressType {
    pub fn resolve(self, listener_class: &ListenerClassSpec) -> AddressType {
        match self {
            PreferredAddressType::HostnameConservative => match listener_class.service_type {
                ServiceType::NodePort => AddressType::Ip,
                _ => AddressType::Hostname,
            },
            PreferredAddressType::Hostname => AddressType::Hostname,
            PreferredAddressType::Ip => AddressType::Ip,
        }
    }
}
