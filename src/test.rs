use k8s_openapi::api::core::v1::{Node, Pod, PodSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
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
        let labels = self.pod.metadata.labels.get_or_insert_with(BTreeMap::new);
        labels.insert(label_key.to_string(), label_value.to_string());

        self
    }

    pub fn with_labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.pod.metadata.labels = Some(labels);
        self
    }

    /// Consumes the Builder and returns a constructed Pod
    pub fn build(&self) -> Pod {
        // We're cloning here because we can't take just `self` in this method because then
        // we couldn't chain the method with the others easily (because they return &mut self and not self)
        self.pod.clone()
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

pub fn build_test_pod(node_name: Option<&str>, labels: Option<BTreeMap<String, String>>) -> Pod {
    Pod {
        metadata: ObjectMeta {
            labels,
            ..ObjectMeta::default()
        },
        spec: Some(PodSpec {
            node_name: node_name.map(|name| name.to_string()),
            ..PodSpec::default()
        }),
        status: None,
    }
}
