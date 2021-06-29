use k8s_openapi::api::core::v1::{Node, Pod, PodCondition, PodSpec, PodStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
use std::collections::BTreeMap;

// TODO: I assume this can also be useful for "real" code, I'm only exposing the fields I really need here though
pub struct PodBuilder {
    pod: Pod,
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        PodBuilder {
            pod: Pod::default(),
        }
    }

    pub fn name(&mut self, name: &str) -> &mut Self {
        self.pod.metadata.name = Some(name.to_string());
        self
    }

    pub fn node_name(&mut self, node_name: &str) -> &mut Self {
        let mut spec = self.pod.spec.get_or_insert_with(PodSpec::default);
        spec.node_name = Some(node_name.to_string());
        self
    }

    pub fn with_label(&mut self, label_key: &str, label_value: &str) -> &mut Self {
        self.pod
            .metadata
            .labels
            .insert(label_key.to_string(), label_value.to_string());
        self
    }

    pub fn with_labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.pod.metadata.labels = labels;
        self
    }

    pub fn phase(&mut self, phase: &str) -> &mut Self {
        let mut status = self.pod.status.get_or_insert_with(PodStatus::default);
        status.phase = Some(phase.to_string());
        self
    }

    pub fn with_condition(&mut self, condition_type: &str, condition_status: &str) -> &mut Self {
        let status = self.pod.status.get_or_insert_with(PodStatus::default);
        let condition = PodCondition {
            status: condition_status.to_string(),
            type_: condition_type.to_string(),
            ..PodCondition::default()
        };
        status.conditions.push(condition);
        self
    }

    pub fn deletion_timestamp(&mut self, deletion_timestamp: Time) -> &mut Self {
        self.pod.metadata.deletion_timestamp = Some(deletion_timestamp);
        self
    }

    /// Consumes the Builder and returns a constructed Pod
    pub fn build(&self) -> Pod {
        // We're cloning here because we can't take just `self` in this method because then
        // we couldn't chain the method with the others easily (because they return &mut self and not self)
        self.pod.clone()
    }
}

pub struct NodeBuilder {
    node: Node,
}

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            node: Node::default(),
        }
    }

    pub fn name(&mut self, name: &str) -> &mut Self {
        self.node.metadata.name = Some(name.to_string());
        self
    }

    /// Consumes the Builder and returns a constructed Node
    pub fn build(&self) -> Node {
        // We're cloning here because we can't take just `self` in this method because then
        // we couldn't chain the method with the others easily (because they return &mut self and not self)
        //
        self.node.clone()
    }
}

pub fn build_test_node(name: &str) -> Node {
    Node {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..ObjectMeta::default()
        },
        spec: None,
        status: None,
    }
}
