use k8s_openapi::{DeepMerge, merge_strategies};

use crate::crd::listener::listeners::v1alpha1::{
    Listener, ListenerIngress, ListenerPort, ListenerSpec, ListenerStatus,
};

impl ListenerSpec {
    pub(super) const fn default_publish_not_ready_addresses() -> Option<bool> {
        Some(true)
    }
}

impl DeepMerge for Listener {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.metadata, other.metadata);
        DeepMerge::merge_from(&mut self.spec, other.spec);
        DeepMerge::merge_from(&mut self.status, other.status);
    }
}

impl DeepMerge for ListenerSpec {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.class_name, other.class_name);
        merge_strategies::map::granular(
            &mut self.extra_pod_selector_labels,
            other.extra_pod_selector_labels,
            |current_item, other_item| {
                DeepMerge::merge_from(current_item, other_item);
            },
        );
        merge_strategies::list::map(
            &mut self.ports,
            other.ports,
            // The unique thing identifying a port is it's name
            &[|lhs, rhs| lhs.name == rhs.name],
            |current_item, other_item| {
                DeepMerge::merge_from(current_item, other_item);
            },
        );
        DeepMerge::merge_from(
            &mut self.publish_not_ready_addresses,
            other.publish_not_ready_addresses,
        );
    }
}

impl DeepMerge for ListenerStatus {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.service_name, other.service_name);
        merge_strategies::list::map(
            &mut self.ingress_addresses,
            other.ingress_addresses,
            // The unique thing identifying an ingress address is it's address
            &[|lhs, rhs| lhs.address == rhs.address],
            |current_item, other_item| {
                DeepMerge::merge_from(current_item, other_item);
            },
        );
        merge_strategies::map::granular(
            &mut self.node_ports,
            other.node_ports,
            |current_item, other_item| {
                DeepMerge::merge_from(current_item, other_item);
            },
        );
    }
}

impl DeepMerge for ListenerIngress {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.address, other.address);
        self.address_type = other.address_type;
        merge_strategies::map::granular(
            &mut self.ports,
            other.ports,
            |current_item, other_item| {
                DeepMerge::merge_from(current_item, other_item);
            },
        );
    }
}

impl DeepMerge for ListenerPort {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.name, other.name);
        DeepMerge::merge_from(&mut self.port, other.port);
        DeepMerge::merge_from(&mut self.protocol, other.protocol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merge_listener() {
        let mut base: ListenerSpec = serde_yaml::from_str(
            "
className: my-listener-class
extraPodSelectorLabels:
  foo: bar
ports:
  - name: http
    port: 8080
    protocol: http
  - name: https
    port: 8080
    protocol: https
# publishNotReadyAddresses defaults to true
",
        )
        .unwrap();

        let patch: ListenerSpec = serde_yaml::from_str(
            "
className: custom-listener-class
extraPodSelectorLabels:
  foo: overridden
  extra: label
ports:
  - name: https
    port: 8443
publishNotReadyAddresses: false
",
        )
        .unwrap();

        base.merge_from(patch);

        let expected: ListenerSpec = serde_yaml::from_str(
            "
className: custom-listener-class
extraPodSelectorLabels:
  foo: overridden
  extra: label
ports:
  - name: http
    port: 8080
    protocol: http
  - name: https
    port: 8443 # overridden
    protocol: https
publishNotReadyAddresses: false
",
        )
        .unwrap();

        assert_eq!(base, expected);
    }
}
