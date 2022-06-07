pub mod container;
pub mod security;
pub mod volume;

use crate::builder::meta::ObjectMetaBuilder;
use crate::error::{Error, OperatorResult};

use k8s_openapi::{
    api::core::v1::{
        Affinity, Container, LocalObjectReference, NodeAffinity, NodeSelector,
        NodeSelectorRequirement, NodeSelectorTerm, Pod, PodAffinity, PodCondition,
        PodSecurityContext, PodSpec, PodStatus, PodTemplateSpec, Toleration, Volume,
    },
    apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement, ObjectMeta},
};
use std::collections::BTreeMap;

/// A builder to build [`Pod`] objects.
///
#[derive(Clone, Default)]
pub struct PodBuilder {
    containers: Vec<Container>,
    host_network: Option<bool>,
    init_containers: Option<Vec<Container>>,
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,
    node_selector: Option<LabelSelector>,
    pod_affinity: Option<PodAffinity>,
    status: Option<PodStatus>,
    security_context: Option<PodSecurityContext>,
    tolerations: Option<Vec<Toleration>>,
    volumes: Option<Vec<Volume>>,
    service_account_name: Option<String>,
    image_pull_secrets: Option<Vec<LocalObjectReference>>,
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        PodBuilder::default()
    }

    pub fn service_account_name(&mut self, value: impl Into<String>) -> &mut Self {
        self.service_account_name = Some(value.into());
        self
    }

    pub fn host_network(&mut self, host_network: bool) -> &mut Self {
        self.host_network = Some(host_network);
        self
    }

    pub fn metadata_default(&mut self) -> &mut Self {
        self.metadata(ObjectMeta::default());
        self
    }

    pub fn metadata_builder<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&mut ObjectMetaBuilder) -> &mut ObjectMetaBuilder,
    {
        let mut builder = ObjectMetaBuilder::new();
        let builder = f(&mut builder);
        self.metadata = Some(builder.build());
        self
    }

    pub fn metadata(&mut self, metadata: impl Into<ObjectMeta>) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn metadata_opt(&mut self, metadata: impl Into<Option<ObjectMeta>>) -> &mut Self {
        self.metadata = metadata.into();
        self
    }

    pub fn node_name(&mut self, node_name: impl Into<String>) -> &mut Self {
        self.node_name = Some(node_name.into());
        self
    }

    pub fn pod_affinity(&mut self, affinity: PodAffinity) -> &mut Self {
        self.pod_affinity = Some(affinity);
        self
    }

    pub fn node_selector(&mut self, node_selector: LabelSelector) -> &mut Self {
        self.node_selector = Some(node_selector);
        self
    }

    pub fn phase(&mut self, phase: &str) -> &mut Self {
        let mut status = self.status.get_or_insert_with(PodStatus::default);
        status.phase = Some(phase.to_string());
        self
    }

    pub fn with_condition(&mut self, condition_type: &str, condition_status: &str) -> &mut Self {
        let status = self.status.get_or_insert_with(PodStatus::default);
        let condition = PodCondition {
            status: condition_status.to_string(),
            type_: condition_type.to_string(),
            ..PodCondition::default()
        };
        status
            .conditions
            .get_or_insert_with(Vec::new)
            .push(condition);
        self
    }

    pub fn add_container(&mut self, container: Container) -> &mut Self {
        self.containers.push(container);
        self
    }

    pub fn add_init_container(&mut self, container: Container) -> &mut Self {
        self.init_containers
            .get_or_insert_with(Vec::new)
            .push(container);
        self
    }

    pub fn add_tolerations(&mut self, tolerations: Vec<Toleration>) -> &mut Self {
        self.tolerations
            .get_or_insert_with(Vec::new)
            .extend(tolerations);
        self
    }

    pub fn security_context(
        &mut self,
        security_context: impl Into<PodSecurityContext>,
    ) -> &mut Self {
        self.security_context = Some(security_context.into());
        self
    }

    pub fn add_volume(&mut self, volume: Volume) -> &mut Self {
        self.volumes.get_or_insert_with(Vec::new).push(volume);
        self
    }

    pub fn add_volumes(&mut self, volumes: Vec<Volume>) -> &mut Self {
        self.volumes.get_or_insert_with(Vec::new).extend(volumes);
        self
    }

    pub fn image_pull_secrets(
        &mut self,
        secrets: impl IntoIterator<Item = String> + Iterator<Item = String>,
    ) -> &mut Self {
        self.image_pull_secrets
            .get_or_insert_with(Vec::new)
            .extend(secrets.map(|s| LocalObjectReference { name: Some(s) }));
        self
    }

    /// Hack because [`Pod`] predates [`LabelSelector`], and so its functionality is split between [`PodSpec::node_selector`] and [`Affinity::node_affinity`]
    fn node_selector_for_label_selector(
        label_selector: Option<LabelSelector>,
    ) -> (Option<BTreeMap<String, String>>, Option<NodeAffinity>) {
        let (node_labels, node_label_exprs) = match label_selector {
            Some(LabelSelector {
                match_labels,
                match_expressions,
            }) => (match_labels, match_expressions),
            None => (None, None),
        };

        let node_affinity = node_label_exprs.map(|node_label_exprs| NodeAffinity {
            required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                node_selector_terms: vec![NodeSelectorTerm {
                    match_expressions: Some(
                        node_label_exprs
                            .into_iter()
                            .map(
                                |LabelSelectorRequirement {
                                     key,
                                     operator,
                                     values,
                                 }| {
                                    NodeSelectorRequirement {
                                        key,
                                        operator,
                                        values,
                                    }
                                },
                            )
                            .collect(),
                    ),
                    ..NodeSelectorTerm::default()
                }],
            }),
            ..NodeAffinity::default()
        });
        (node_labels, node_affinity)
    }

    fn build_spec(&self) -> PodSpec {
        let (node_selector_labels, node_affinity) =
            Self::node_selector_for_label_selector(self.node_selector.clone());
        PodSpec {
            containers: self.containers.clone(),
            host_network: self.host_network,
            init_containers: self.init_containers.clone(),
            node_name: self.node_name.clone(),
            node_selector: node_selector_labels,
            affinity: node_affinity
                .map(|node_affinity| Affinity {
                    node_affinity: Some(node_affinity),
                    pod_affinity: self.pod_affinity.clone(),
                    ..Affinity::default()
                })
                .or_else(|| {
                    Some(Affinity {
                        pod_affinity: self.pod_affinity.clone(),
                        ..Affinity::default()
                    })
                }),
            security_context: self.security_context.clone(),
            tolerations: self.tolerations.clone(),
            volumes: self.volumes.clone(),
            // Legacy feature for ancient Docker images
            // In practice, this just causes a bunch of unused environment variables that may conflict with other uses,
            // such as https://github.com/stackabletech/spark-operator/pull/256.
            enable_service_links: Some(false),
            service_account_name: self.service_account_name.clone(),
            image_pull_secrets: self.image_pull_secrets.clone(),
            ..PodSpec::default()
        }
    }

    /// Consumes the Builder and returns a constructed [`Pod`]
    pub fn build(&self) -> OperatorResult<Pod> {
        Ok(Pod {
            metadata: match self.metadata {
                None => return Err(Error::MissingObjectKey { key: "metadata" }),
                Some(ref metadata) => metadata.clone(),
            },
            spec: Some(self.build_spec()),
            status: self.status.clone(),
        })
    }

    /// Returns a [`PodTemplateSpec`], usable for building a [`StatefulSet`](`k8s_openapi::api::apps::v1::StatefulSet`)
    /// or [`Deployment`](`k8s_openapi::api::apps::v1::Deployment`)
    pub fn build_template(&self) -> PodTemplateSpec {
        if self.status.is_some() {
            tracing::warn!("Tried building a PodTemplate for a PodBuilder with a status, the status will be ignored...");
        }
        PodTemplateSpec {
            metadata: self.metadata.clone(),
            spec: Some(self.build_spec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{
        meta::ObjectMetaBuilder,
        pod::{container::ContainerBuilder, volume::VolumeBuilder, PodBuilder},
    };
    use k8s_openapi::{
        api::core::v1::{LocalObjectReference, PodAffinity, PodAffinityTerm},
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    };

    #[test]
    fn test_pod_builder() {
        let container = ContainerBuilder::new("containername")
            .image("stackable/zookeeper:2.4.14")
            .command(vec!["zk-server-start.sh".to_string()])
            .args(vec!["stackable/conf/zk.properties".to_string()])
            .add_volume_mount("zk-worker-1", "conf/")
            .build();

        let init_container = ContainerBuilder::new("init_containername")
            .image("stackable/zookeeper:2.4.14")
            .command(vec!["wrapper.sh".to_string()])
            .args(vec!["12345".to_string()])
            .build();

        let pod_affinity = PodAffinity {
            preferred_during_scheduling_ignored_during_execution: None,
            required_during_scheduling_ignored_during_execution: Some(vec![PodAffinityTerm {
                label_selector: Some(LabelSelector {
                    match_expressions: Some(vec![LabelSelectorRequirement {
                        key: "security".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["S1".to_string()]),
                    }]),
                    match_labels: None,
                }),
                topology_key: "topology.kubernetes.io/zone".to_string(),
                ..Default::default()
            }]),
        };

        let pod = PodBuilder::new()
            .pod_affinity(pod_affinity.clone())
            .metadata(ObjectMetaBuilder::new().name("testpod").build())
            .add_container(container)
            .add_init_container(init_container)
            .node_name("worker-1.stackable.demo")
            .add_volume(
                VolumeBuilder::new("zk-worker-1")
                    .with_config_map("configmap")
                    .build(),
            )
            .build()
            .unwrap();

        let pod_spec = pod.spec.unwrap();

        assert_eq!(pod_spec.affinity.unwrap().pod_affinity, Some(pod_affinity));
        assert_eq!(pod.metadata.name.unwrap(), "testpod");
        assert_eq!(
            pod_spec.node_name.as_ref().unwrap(),
            "worker-1.stackable.demo"
        );
        assert_eq!(pod_spec.init_containers.as_ref().unwrap().len(), 1);
        assert_eq!(
            pod_spec
                .init_containers
                .as_ref()
                .and_then(|containers| containers.get(0).as_ref().map(|c| c.name.clone())),
            Some("init_containername".to_string())
        );

        assert_eq!(
            pod_spec.volumes.as_ref().and_then(|volumes| volumes
                .get(0)
                .as_ref()
                .and_then(|volume| volume.config_map.as_ref()?.name.clone())),
            Some("configmap".to_string())
        );

        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("foo"))
            .build()
            .unwrap();

        assert_eq!(pod.metadata.name.unwrap(), "foo");
    }

    #[test]
    fn test_pod_builder_image_pull_secrets() {
        let container = ContainerBuilder::new("container")
            .image("private-comapany/product:2.4.14")
            .build();

        let pod = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("testpod").build())
            .add_container(container)
            .image_pull_secrets(vec!["company-registry-secret".to_string()].into_iter())
            .build()
            .unwrap();

        assert_eq!(
            pod.spec.unwrap().image_pull_secrets.unwrap(),
            vec![LocalObjectReference {
                name: Some("company-registry-secret".to_string())
            }]
        );
    }
}
