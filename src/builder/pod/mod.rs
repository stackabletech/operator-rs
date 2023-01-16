pub mod container;
pub mod security;
pub mod volume;

use crate::builder::meta::ObjectMetaBuilder;
use crate::commons::product_image_selection::ResolvedProductImage;
use crate::error::{Error, OperatorResult};

use super::{ListenerOperatorVolumeSourceBuilder, ListenerReference, VolumeBuilder};
use k8s_openapi::{
    api::core::v1::{
        Affinity, Container, LocalObjectReference, NodeAffinity, NodeSelector,
        NodeSelectorRequirement, NodeSelectorTerm, Pod, PodAffinity, PodAntiAffinity, PodCondition,
        PodSecurityContext, PodSpec, PodStatus, PodTemplateSpec, Toleration, Volume,
    },
    apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement, ObjectMeta},
};
use std::collections::BTreeMap;

/// A builder to build [`Pod`] or [`PodTemplateSpec`] objects.
#[derive(Clone, Default)]
pub struct PodBuilder {
    containers: Vec<Container>,
    host_network: Option<bool>,
    init_containers: Option<Vec<Container>>,
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,
    node_selector: Option<LabelSelector>,
    pod_affinity: Option<PodAffinity>,
    pod_anti_affinity: Option<PodAntiAffinity>,
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

    pub fn pod_affinity_opt(&mut self, affinity: Option<PodAffinity>) -> &mut Self {
        self.pod_affinity = affinity;
        self
    }

    pub fn pod_anti_affinity(&mut self, anti_affinity: PodAntiAffinity) -> &mut Self {
        self.pod_anti_affinity = Some(anti_affinity);
        self
    }

    pub fn pod_anti_affinity_opt(&mut self, anti_affinity: Option<PodAntiAffinity>) -> &mut Self {
        self.pod_anti_affinity = anti_affinity;
        self
    }

    pub fn node_selector(&mut self, node_selector: LabelSelector) -> &mut Self {
        self.node_selector = Some(node_selector);
        self
    }

    pub fn node_selector_opt(&mut self, node_selector: Option<LabelSelector>) -> &mut Self {
        self.node_selector = node_selector;
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

    /// Utility function for the common case of adding an empty dir Volume
    /// with the given name and no medium and no quantity.
    pub fn add_volume_with_empty_dir(&mut self, name: impl Into<String>) -> &mut Self {
        self.add_volume(
            VolumeBuilder::new(name)
                .with_empty_dir(None::<String>, None)
                .build(),
        )
    }

    pub fn add_volumes(&mut self, volumes: Vec<Volume>) -> &mut Self {
        self.volumes.get_or_insert_with(Vec::new).extend(volumes);
        self
    }

    /// Add a [`Volume`] for the storage class `listeners.stackable.tech` with the given listener
    /// class.
    ///
    /// # Example
    ///
    /// ```
    /// # use stackable_operator::builder::PodBuilder;
    /// # use stackable_operator::builder::ContainerBuilder;
    /// let pod = PodBuilder::new()
    ///     .metadata_default()
    ///     .add_container(
    ///         ContainerBuilder::new("container")
    ///             .unwrap()
    ///             .add_volume_mount("listener", "/path/to/volume")
    ///             .build(),
    ///     )
    ///     .add_listener_volume_by_listener_class("listener", "nodeport")
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!("\
    /// apiVersion: v1
    /// kind: Pod
    /// metadata: {}
    /// spec:
    ///   affinity: {}
    ///   containers:
    ///   - name: container
    ///     volumeMounts:
    ///     - mountPath: /path/to/volume
    ///       name: listener
    ///   enableServiceLinks: false
    ///   volumes:
    ///   - ephemeral:
    ///       volumeClaimTemplate:
    ///         metadata:
    ///           annotations:
    ///             listeners.stackable.tech/listener-class: nodeport
    ///         spec:
    ///           accessModes:
    ///           - ReadWriteMany
    ///           resources:
    ///             requests:
    ///               storage: '1'
    ///           storageClassName: listeners.stackable.tech
    ///     name: listener
    /// ", serde_yaml::to_string(&pod).unwrap())
    /// ```
    pub fn add_listener_volume_by_listener_class(
        &mut self,
        volume_name: &str,
        listener_class: &str,
    ) -> &mut Self {
        self.add_volume(Volume {
            name: volume_name.into(),
            ephemeral: Some(
                ListenerOperatorVolumeSourceBuilder::new(&ListenerReference::ListenerClass(
                    listener_class.into(),
                ))
                .build(),
            ),
            ..Volume::default()
        });
        self
    }

    /// Add a [`Volume`] for the storage class `listeners.stackable.tech` with the given listener
    /// name.
    ///
    /// # Example
    ///
    /// ```
    /// # use stackable_operator::builder::PodBuilder;
    /// # use stackable_operator::builder::ContainerBuilder;
    /// let pod = PodBuilder::new()
    ///     .metadata_default()
    ///     .add_container(
    ///         ContainerBuilder::new("container")
    ///             .unwrap()
    ///             .add_volume_mount("listener", "/path/to/volume")
    ///             .build(),
    ///     )
    ///     .add_listener_volume_by_listener_name("listener", "preprovisioned-listener")
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!("\
    /// apiVersion: v1
    /// kind: Pod
    /// metadata: {}
    /// spec:
    ///   affinity: {}
    ///   containers:
    ///   - name: container
    ///     volumeMounts:
    ///     - mountPath: /path/to/volume
    ///       name: listener
    ///   enableServiceLinks: false
    ///   volumes:
    ///   - ephemeral:
    ///       volumeClaimTemplate:
    ///         metadata:
    ///           annotations:
    ///             listeners.stackable.tech/listener-name: preprovisioned-listener
    ///         spec:
    ///           accessModes:
    ///           - ReadWriteMany
    ///           resources:
    ///             requests:
    ///               storage: '1'
    ///           storageClassName: listeners.stackable.tech
    ///     name: listener
    /// ", serde_yaml::to_string(&pod).unwrap())
    /// ```
    pub fn add_listener_volume_by_listener_name(
        &mut self,
        volume_name: &str,
        listener_name: &str,
    ) -> &mut Self {
        self.add_volume(Volume {
            name: volume_name.into(),
            ephemeral: Some(
                ListenerOperatorVolumeSourceBuilder::new(&ListenerReference::ListenerName(
                    listener_name.into(),
                ))
                .build(),
            ),
            ..Volume::default()
        });
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

    /// Extend the pod's image_pull_secrets field with the pull secrets from a given [ResolvedProductImage]
    pub fn image_pull_secrets_from_product_image(
        &mut self,
        product_image: &ResolvedProductImage,
    ) -> &mut Self {
        if let Some(pull_secrets) = &product_image.pull_secrets {
            self.image_pull_secrets
                .get_or_insert_with(Vec::new)
                .extend_from_slice(pull_secrets);
        }
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
            affinity: Some(Affinity {
                node_affinity,
                pod_affinity: self.pod_affinity.clone(),
                pod_anti_affinity: self.pod_anti_affinity.clone(),
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
    use super::*;
    use crate::builder::{
        meta::ObjectMetaBuilder,
        pod::{container::ContainerBuilder, volume::VolumeBuilder},
    };
    use k8s_openapi::{
        api::core::v1::{LocalObjectReference, PodAffinity, PodAffinityTerm},
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    };
    use rstest::*;

    // A simple [`Container`] with a name and image.
    #[fixture]
    fn dummy_container() -> Container {
        ContainerBuilder::new("container")
            .expect("ContainerBuilder not created")
            .image("private-company/product:2.4.14")
            .build()
    }

    /// A [`PodBuilder`] that already contains the minum setup to build a Pod (name and container).
    #[fixture]
    fn pod_builder_with_name_and_container(dummy_container: Container) -> PodBuilder {
        let mut builder = PodBuilder::new();
        builder
            .metadata(ObjectMetaBuilder::new().name("testpod").build())
            .add_container(dummy_container);
        builder
    }

    // A fixture for a node selector to use on a Pod, and the resulting node selector labels and node affinity.
    #[fixture]
    fn node_selector1() -> (
        LabelSelector,
        Option<BTreeMap<String, String>>,
        Option<NodeAffinity>,
    ) {
        let labels = BTreeMap::from([("key".to_owned(), "value".to_owned())]);
        let label_selector = LabelSelector {
            match_expressions: Some(vec![LabelSelectorRequirement {
                key: "security".to_owned(),
                operator: "In".to_owned(),
                values: Some(vec!["S1".to_owned(), "S2".to_owned()]),
            }]),
            match_labels: Some(labels.clone()),
        };
        let affinity = Some(NodeAffinity {
            required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                node_selector_terms: vec![NodeSelectorTerm {
                    match_expressions: Some(vec![NodeSelectorRequirement {
                        key: "security".to_owned(),
                        operator: "In".to_owned(),
                        values: Some(vec!["S1".to_owned(), "S2".to_owned()]),
                    }]),
                    ..Default::default()
                }],
            }),
            ..Default::default()
        });
        (label_selector, Some(labels), affinity)
    }

    #[fixture]
    fn pod_affinity() -> PodAffinity {
        PodAffinity {
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
        }
    }

    #[fixture]
    fn pod_anti_affinity(pod_affinity: PodAffinity) -> PodAntiAffinity {
        PodAntiAffinity {
            preferred_during_scheduling_ignored_during_execution: None,
            required_during_scheduling_ignored_during_execution: pod_affinity
                .required_during_scheduling_ignored_during_execution,
        }
    }

    #[rstest]
    fn test_pod_builder_pod_name() {
        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("foo"))
            .build()
            .unwrap();

        assert_eq!(pod.metadata.name.unwrap(), "foo");
    }

    #[rstest]
    fn test_pod_builder(pod_affinity: PodAffinity, dummy_container: Container) {
        let init_container = ContainerBuilder::new("init-containername")
            .expect("ContainerBuilder not created")
            .image("stackable/zookeeper:2.4.14")
            .build();

        let pod = PodBuilder::new()
            .pod_affinity(pod_affinity.clone())
            .metadata(ObjectMetaBuilder::new().name("testpod").build())
            .add_container(dummy_container)
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
            Some("init-containername".to_string())
        );

        assert_eq!(
            pod_spec.volumes.as_ref().and_then(|volumes| volumes
                .get(0)
                .as_ref()
                .and_then(|volume| volume.config_map.as_ref()?.name.clone())),
            Some("configmap".to_string())
        );
    }

    #[rstest]
    fn test_pod_builder_image_pull_secrets(mut pod_builder_with_name_and_container: PodBuilder) {
        let pod = pod_builder_with_name_and_container
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

    /// Test if setting a node selector generates the correct node selector labels and node affinity on the Pod.
    #[rstest]
    fn test_pod_builder_node_selector(
        mut pod_builder_with_name_and_container: PodBuilder,
        node_selector1: (
            LabelSelector,
            Option<BTreeMap<String, String>>,
            Option<NodeAffinity>,
        ),
    ) {
        // destruct fixture
        let (node_selector, expected_labels, expected_affinity) = node_selector1;
        // first test with the normal node_selector function
        let pod = pod_builder_with_name_and_container
            .clone()
            .node_selector(node_selector.clone())
            .build()
            .unwrap();

        let spec = pod.spec.unwrap();
        assert_eq!(spec.node_selector, expected_labels);
        assert_eq!(spec.affinity.unwrap().node_affinity, expected_affinity);

        // test the node_selector_opt function
        let pod = pod_builder_with_name_and_container
            .node_selector_opt(Some(node_selector))
            .build()
            .unwrap();

        // asserts
        let spec = pod.spec.unwrap();
        assert_eq!(spec.node_selector, expected_labels);
        assert_eq!(spec.affinity.unwrap().node_affinity, expected_affinity);
    }

    /// Test if setting a node selector generates the correct node selector labels and node affinity on the Pod,
    /// while keeping the manually set Pod affinities. Since they are mangled together, it makes sense to make sure that
    /// one is not replacing the other
    #[rstest]
    fn test_pod_builder_node_selector_and_affinity(
        mut pod_builder_with_name_and_container: PodBuilder,
        node_selector1: (
            LabelSelector,
            Option<BTreeMap<String, String>>,
            Option<NodeAffinity>,
        ),
        pod_affinity: PodAffinity,
        pod_anti_affinity: PodAntiAffinity,
    ) {
        // destruct fixture
        let (node_selector, expected_labels, expected_affinity) = node_selector1;
        // first test with the normal functions
        let pod = pod_builder_with_name_and_container
            .clone()
            .node_selector(node_selector.clone())
            .pod_affinity(pod_affinity.clone())
            .pod_anti_affinity(pod_anti_affinity.clone())
            .build()
            .unwrap();

        let spec = pod.spec.unwrap();
        assert_eq!(spec.node_selector, expected_labels);
        let affinity = spec.affinity.unwrap();
        assert_eq!(affinity.node_affinity, expected_affinity);
        assert_eq!(affinity.pod_affinity, Some(pod_affinity.clone()));
        assert_eq!(affinity.pod_anti_affinity, Some(pod_anti_affinity.clone()));

        // test the *_opt functions
        let pod = pod_builder_with_name_and_container
            .node_selector_opt(Some(node_selector))
            .pod_affinity_opt(Some(pod_affinity.clone()))
            .pod_anti_affinity_opt(Some(pod_anti_affinity.clone()))
            .build()
            .unwrap();

        // asserts
        let spec = pod.spec.unwrap();
        assert_eq!(spec.node_selector, expected_labels);
        let affinity = spec.affinity.unwrap();
        assert_eq!(affinity.node_affinity, expected_affinity);
        assert_eq!(affinity.pod_affinity, Some(pod_affinity));
        assert_eq!(affinity.pod_anti_affinity, Some(pod_anti_affinity));
    }
}
