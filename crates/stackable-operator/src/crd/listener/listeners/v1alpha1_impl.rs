use crate::crd::listener::listeners::v1alpha1::{
    Listener, ListenerIngress, ListenerPort, ListenerSpec, ListenerStatus,
};

impl ListenerSpec {
    pub(super) const fn default_publish_not_ready_addresses() -> Option<bool> {
        Some(true)
    }
}

impl k8s_openapi::DeepMerge for Listener {
    fn merge_from(&mut self, other: Self) {
        k8s_openapi::DeepMerge::merge_from(&mut self.metadata, other.metadata);
        k8s_openapi::DeepMerge::merge_from(&mut self.spec, other.spec);
        k8s_openapi::DeepMerge::merge_from(&mut self.status, other.status);
    }
}

impl k8s_openapi::DeepMerge for ListenerSpec {
    fn merge_from(&mut self, other: Self) {
        k8s_openapi::DeepMerge::merge_from(&mut self.class_name, other.class_name);
        k8s_openapi::merge_strategies::map::granular(
            &mut self.extra_pod_selector_labels,
            other.extra_pod_selector_labels,
            |current_item, other_item| {
                k8s_openapi::DeepMerge::merge_from(current_item, other_item);
            },
        );
        k8s_openapi::merge_strategies::list::map(
            &mut self.ports,
            other.ports,
            &[|lhs, rhs| lhs.name == rhs.name],
            |current_item, other_item| {
                k8s_openapi::DeepMerge::merge_from(current_item, other_item);
            },
        );
        k8s_openapi::DeepMerge::merge_from(
            &mut self.publish_not_ready_addresses,
            other.publish_not_ready_addresses,
        );
        todo!()
    }
}

impl k8s_openapi::DeepMerge for ListenerStatus {
    fn merge_from(&mut self, other: Self) {
        k8s_openapi::DeepMerge::merge_from(&mut self.service_name, other.service_name);
        k8s_openapi::merge_strategies::list::map(
            &mut self.ingress_addresses,
            other.ingress_addresses,
            &[|lhs, rhs| lhs.address == rhs.address],
            |current_item, other_item| {
                k8s_openapi::DeepMerge::merge_from(current_item, other_item);
            },
        );
        k8s_openapi::merge_strategies::map::granular(
            &mut self.node_ports,
            other.node_ports,
            |current_item, other_item| {
                k8s_openapi::DeepMerge::merge_from(current_item, other_item);
            },
        );
    }
}

impl k8s_openapi::DeepMerge for ListenerIngress {
    fn merge_from(&mut self, other: Self) {
        k8s_openapi::DeepMerge::merge_from(&mut self.address, other.address);
        self.address_type = other.address_type;
        k8s_openapi::merge_strategies::map::granular(
            &mut self.ports,
            other.ports,
            |current_item, other_item| {
                k8s_openapi::DeepMerge::merge_from(current_item, other_item);
            },
        );
    }
}

impl k8s_openapi::DeepMerge for ListenerPort {
    fn merge_from(&mut self, other: Self) {
        k8s_openapi::DeepMerge::merge_from(&mut self.name, other.name);
        k8s_openapi::DeepMerge::merge_from(&mut self.port, other.port);
        k8s_openapi::DeepMerge::merge_from(&mut self.protocol, other.protocol);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn deep_merge_listener() {
        todo!("Add some basic tests for merging");
    }
}
