use std::{collections::BTreeMap, num::TryFromIntError};

use indexmap::IndexMap;
use k8s_openapi::{
    api::core::v1::{
        Affinity, Container, LocalObjectReference, NodeAffinity, Pod, PodAffinity, PodAntiAffinity,
        PodCondition, PodSecurityContext, PodSpec, PodStatus, PodTemplateSpec,
        ResourceRequirements, Toleration, Volume,
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
};
use snafu::{OptionExt, ResultExt, Snafu};
use tracing::{instrument, warn};

use crate::kvp::Labels;
use crate::{
    builder::meta::ObjectMetaBuilder,
    commons::{
        affinity::StackableAffinity,
        product_image_selection::ResolvedProductImage,
        resources::{
            ComputeResource, ResourceRequirementsExt, ResourceRequirementsType,
            LIMIT_REQUEST_RATIO_CPU, LIMIT_REQUEST_RATIO_MEMORY,
        },
    },
    time::Duration,
};

use self::volume::{ListenerOperatorVolumeSourceBuilder, ListenerReference, VolumeBuilder};

pub mod container;
pub mod resources;
pub mod security;
pub mod volume;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("termination grace period is too long (got {duration}, maximum allowed is {max})", max = Duration::from_secs(i64::MAX as u64)))]
    TerminationGracePeriodTooLong {
        source: TryFromIntError,
        duration: Duration,
    },

    #[snafu(display("failed to add listener volume {name:?} to the pod"))]
    ListenerVolume {
        source: volume::ListenerOperatorVolumeSourceBuilderError,
        name: String,
    },

    #[snafu(display("object is missing key {key:?}"))]
    MissingObjectKey { key: &'static str },

    #[snafu(display(
        "Colliding volume name {colliding_volume_name:?} in volumes with different content"
    ))]
    VolumeNameCollision { colliding_volume_name: String },
}

/// A builder to build [`Pod`] or [`PodTemplateSpec`] objects.
///
/// This struct is often times using an [`IndexMap`] to have consistent ordering (so we don't produce reconcile loops).
/// We are also choosing it over a [`BTreeMap`], because it is easier to debug for users, as logically grouped volumes
/// (e.g. all volumes related to S3) are near each other in the list instead of "just" being sorted alphabetically.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PodBuilder {
    containers: Vec<Container>,
    host_network: Option<bool>,
    init_containers: Option<Vec<Container>>,
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,
    node_selector: Option<BTreeMap<String, String>>,
    node_affinity: Option<NodeAffinity>,
    pod_affinity: Option<PodAffinity>,
    pod_anti_affinity: Option<PodAntiAffinity>,
    status: Option<PodStatus>,
    security_context: Option<PodSecurityContext>,
    tolerations: Option<Vec<Toleration>>,

    /// The key is the volume name.
    volumes: IndexMap<String, Volume>,
    service_account_name: Option<String>,
    image_pull_secrets: Option<Vec<LocalObjectReference>>,
    restart_policy: Option<String>,
    termination_grace_period_seconds: Option<i64>,
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

    pub fn node_affinity(&mut self, affinity: NodeAffinity) -> &mut Self {
        self.node_affinity = Some(affinity);
        self
    }

    pub fn node_affinity_opt(&mut self, affinity: Option<NodeAffinity>) -> &mut Self {
        self.node_affinity = affinity;
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

    pub fn node_selector(&mut self, node_selector: BTreeMap<String, String>) -> &mut Self {
        self.node_selector = Some(node_selector);
        self
    }

    pub fn node_selector_opt(
        &mut self,
        node_selector: Option<BTreeMap<String, String>>,
    ) -> &mut Self {
        self.node_selector = node_selector;
        self
    }

    pub fn affinity(&mut self, affinities: &StackableAffinity) -> &mut Self {
        self.pod_affinity.clone_from(&affinities.pod_affinity);
        self.pod_anti_affinity
            .clone_from(&affinities.pod_anti_affinity);
        self.node_affinity.clone_from(&affinities.node_affinity);
        self.node_selector = affinities.node_selector.clone().map(|ns| ns.node_selector);
        self
    }

    pub fn phase(&mut self, phase: &str) -> &mut Self {
        let status = self.status.get_or_insert_with(PodStatus::default);
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

    /// Add the given init container.
    /// If no resources are set, we set a default limit of 10m CPU and 128Mi
    /// memory. Request values are set to the same values as the limits.
    pub fn add_init_container(&mut self, mut container: Container) -> &mut Self {
        // https://github.com/stackabletech/issues/issues/368:
        // We only set default limits on *init* containers, as they normally
        // simply copy stuff around, do some text replacement or, at a maximum,
        // create a tls truststore.These operations should normally complete
        // in <= 1s, so worst-case the Pod will take 1-2s longer to start up
        // when the default is too low. However, things are different with
        // sidecars, where e.g. a bundle builder, metric collector or a vector
        // log sidecar can be overloaded and slow down operations or cause
        // missing data, e.g. metrics or logs. Hence we don't apply any defaults
        // for sidecars, product operators have to explicitly make a decision
        // on what the resource limits should be.

        // FIXME: These defaults should not live here and should
        // instead be set inside the container builder. Having container types
        // should greatly simplify setting the default resource requirements.
        // This method should instead be "as dumb" as it can be and should
        // simply add the provided container to the internal vector. The problem
        // with the solution down below is that wie side-step the common
        // interface provided by the `with_resource`, `with_cpu` and
        // `with_memory` methods of the builder.

        if container.resources.is_none() {
            let limits = Some(BTreeMap::from([
                ("cpu".to_string(), Quantity("10m".to_string())),
                ("memory".to_string(), Quantity("128Mi".to_string())),
            ]));
            container.resources = Some(ResourceRequirements {
                limits: limits.clone(),
                requests: limits,
                ..ResourceRequirements::default()
            });
        }

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

    /// Utility function for the common case of adding an emptyDir Volume
    /// with the given name and no medium and no quantity.
    pub fn add_empty_dir_volume(
        &mut self,
        name: impl Into<String>,
        quantity: Option<Quantity>,
    ) -> Result<&mut Self> {
        self.add_volume(
            VolumeBuilder::new(name)
                .with_empty_dir(None::<String>, quantity)
                .build(),
        )
    }

    /// Adds a new [`Volume`] to the container while ensuring that no colliding [`Volume`] exists.
    ///
    /// A colliding [`Volume`] would have the same name but a different content than another
    /// [`Volume`]. An appropriate error is returned when such a colliding volume name is
    /// encountered.
    ///
    /// ### Note
    ///
    /// Previously, this function unconditionally added [`Volume`]s, which resulted in invalid
    /// [`PodSpec`]s.
    #[instrument(skip(self))]
    pub fn add_volume(&mut self, volume: Volume) -> Result<&mut Self> {
        if let Some(existing_volume) = self.volumes.get(&volume.name) {
            if existing_volume != &volume {
                let colliding_volume_name = &volume.name;
                // We don't want to include the details in the error message, but instead trace them
                tracing::error!(
                    colliding_volume_name,
                    ?existing_volume,
                    "Colliding volume name in volumes with different content"
                );

                VolumeNameCollisionSnafu {
                    colliding_volume_name,
                }
                .fail()?;
            }
        } else {
            self.volumes.insert(volume.name.clone(), volume);
        }

        Ok(self)
    }

    /// See [`Self::add_volume`] for details
    pub fn add_volumes(&mut self, volumes: Vec<Volume>) -> Result<&mut Self> {
        for volume in volumes {
            self.add_volume(volume)?;
        }

        Ok(self)
    }

    /// Add a [`Volume`] for the storage class `listeners.stackable.tech` with the given listener
    /// class.
    ///
    /// # Example
    ///
    /// ```
    /// # use stackable_operator::builder::pod::PodBuilder;
    /// # use stackable_operator::builder::pod::container::ContainerBuilder;
    /// # use stackable_operator::kvp::Labels;
    /// # use k8s_openapi::{
    /// #     apimachinery::pkg::apis::meta::v1::ObjectMeta,
    /// # };
    /// # use std::collections::BTreeMap;
    ///
    /// let labels: Labels = Labels::try_from(
    ///        BTreeMap::from([("app.kubernetes.io/component", "test-role"),
    ///             ("app.kubernetes.io/instance", "test"),
    ///             ("app.kubernetes.io/name", "test")]))
    /// .unwrap();
    ///
    /// let pod = PodBuilder::new()
    ///     .metadata_default()
    ///     .add_container(
    ///         ContainerBuilder::new("container")
    ///             .unwrap()
    ///             .add_volume_mount("listener", "/path/to/volume")
    ///             .unwrap()
    ///             .build(),
    ///     )
    ///     .add_listener_volume_by_listener_class("listener", "nodeport", &labels)
    ///     .unwrap()
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
    ///           labels:
    ///             app.kubernetes.io/component: test-role
    ///             app.kubernetes.io/instance: test
    ///             app.kubernetes.io/name: test
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
        labels: &Labels,
    ) -> Result<&mut Self> {
        let listener_reference = ListenerReference::ListenerClass(listener_class.to_string());
        let volume = ListenerOperatorVolumeSourceBuilder::new(&listener_reference, labels)
            .context(ListenerVolumeSnafu { name: volume_name })?
            .build_ephemeral()
            .context(ListenerVolumeSnafu { name: volume_name })?;

        self.add_volume(Volume {
            name: volume_name.into(),
            ephemeral: Some(volume),
            ..Volume::default()
        })?;

        Ok(self)
    }

    /// Add a [`Volume`] for the storage class `listeners.stackable.tech` with the given listener
    /// name.
    ///
    /// # Example
    ///
    /// ```
    /// # use stackable_operator::builder::pod::PodBuilder;
    /// # use stackable_operator::builder::pod::container::ContainerBuilder;
    /// # use stackable_operator::kvp::Labels;
    /// # use k8s_openapi::{
    /// #    apimachinery::pkg::apis::meta::v1::ObjectMeta,
    /// # };
    /// # use std::collections::BTreeMap;
    ///
    /// let labels: Labels = Labels::try_from(
    ///        BTreeMap::from([("app.kubernetes.io/component", "test-role"),
    ///             ("app.kubernetes.io/instance", "test"),
    ///             ("app.kubernetes.io/name", "test")]))
    /// .unwrap();
    ///
    /// let pod = PodBuilder::new()
    ///     .metadata_default()
    ///     .add_container(
    ///         ContainerBuilder::new("container")
    ///             .unwrap()
    ///             .add_volume_mount("listener", "/path/to/volume")
    ///             .unwrap()
    ///             .build(),
    ///     )
    ///     .add_listener_volume_by_listener_name("listener", "preprovisioned-listener", &labels)
    ///     .unwrap()
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
    ///           labels:
    ///             app.kubernetes.io/component: test-role
    ///             app.kubernetes.io/instance: test
    ///             app.kubernetes.io/name: test
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
        labels: &Labels,
    ) -> Result<&mut Self> {
        let listener_reference = ListenerReference::ListenerName(listener_name.to_string());
        let volume = ListenerOperatorVolumeSourceBuilder::new(&listener_reference, labels)
            .context(ListenerVolumeSnafu { name: volume_name })?
            .build_ephemeral()
            .context(ListenerVolumeSnafu { name: volume_name })?;

        self.add_volume(Volume {
            name: volume_name.into(),
            ephemeral: Some(volume),
            ..Volume::default()
        })?;

        Ok(self)
    }

    pub fn image_pull_secrets(
        &mut self,
        secrets: impl IntoIterator<Item = String> + Iterator<Item = String>,
    ) -> &mut Self {
        self.image_pull_secrets
            .get_or_insert_with(Vec::new)
            .extend(secrets.map(|name| LocalObjectReference { name }));
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

    pub fn restart_policy(&mut self, restart_policy: &str) -> &mut Self {
        self.restart_policy = Some(String::from(restart_policy));
        self
    }

    pub fn termination_grace_period(
        &mut self,
        termination_grace_period: &Duration,
    ) -> Result<&mut Self> {
        let termination_grace_period_seconds = termination_grace_period
            .as_secs()
            .try_into()
            .context(TerminationGracePeriodTooLongSnafu {
                duration: *termination_grace_period,
            })?;

        self.termination_grace_period_seconds = Some(termination_grace_period_seconds);
        Ok(self)
    }

    /// Returns a constructed [`Pod`]
    pub fn build(&self) -> Result<Pod> {
        let metadata = self
            .metadata
            .clone()
            .context(MissingObjectKeySnafu { key: "metadata" })?;
        Ok(Pod {
            metadata,
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

    fn build_spec(&self) -> PodSpec {
        let volumes = if self.volumes.is_empty() {
            None
        } else {
            Some(self.volumes.values().cloned().collect())
        };

        let pod_spec = PodSpec {
            containers: self.containers.clone(),
            host_network: self.host_network,
            init_containers: self.init_containers.clone(),
            node_name: self.node_name.clone(),
            node_selector: self.node_selector.clone(),
            affinity: Some(Affinity {
                node_affinity: self.node_affinity.clone(),
                pod_affinity: self.pod_affinity.clone(),
                pod_anti_affinity: self.pod_anti_affinity.clone(),
            }),
            security_context: self.security_context.clone(),
            tolerations: self.tolerations.clone(),
            volumes,
            // Legacy feature for ancient Docker images
            // In practice, this just causes a bunch of unused environment variables that may conflict with other uses,
            // such as https://github.com/stackabletech/spark-operator/pull/256.
            enable_service_links: Some(false),
            service_account_name: self.service_account_name.clone(),
            image_pull_secrets: self.image_pull_secrets.clone(),
            restart_policy: self.restart_policy.clone(),
            termination_grace_period_seconds: self.termination_grace_period_seconds,
            ..PodSpec::default()
        };

        // We don't hard error here, because if we do, the StatefulSet (for
        // example) doesn't show up at all. Instead users then need to comb
        // through the logs to find the error. That's why we opted to just
        // throw a warning which will get displayed in the Kubernetes
        // status. Additionally the Statefulset will have events describing the
        // actual problem.

        pod_spec
            .check_resource_requirement(ResourceRequirementsType::Limits, "cpu")
            .unwrap_or_else(|err| warn!("{}", err));

        pod_spec
            .check_resource_requirement(ResourceRequirementsType::Limits, "memory")
            .unwrap_or_else(|err| warn!("{}", err));

        pod_spec
            .check_limit_to_request_ratio(&ComputeResource::Cpu, LIMIT_REQUEST_RATIO_CPU)
            .unwrap_or_else(|err| warn!("{}", err));

        pod_spec
            .check_limit_to_request_ratio(&ComputeResource::Memory, LIMIT_REQUEST_RATIO_MEMORY)
            .unwrap_or_else(|err| warn!("{}", err));

        pod_spec
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::{
        api::core::v1::{LocalObjectReference, PodAffinity, PodAffinityTerm},
        apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
    };
    use rstest::*;

    use crate::builder::{
        meta::ObjectMetaBuilder,
        pod::{
            container::ContainerBuilder, resources::ResourceRequirementsBuilder,
            volume::VolumeBuilder,
        },
    };

    use super::*;

    // A simple [`Container`] with a name and image.
    #[fixture]
    fn dummy_container() -> Container {
        let resources = ResourceRequirementsBuilder::new()
            .with_cpu_request("1")
            .with_cpu_limit("1")
            .with_memory_request("128Mi")
            .with_memory_limit("128Mi")
            .build();

        ContainerBuilder::new("container")
            .expect("ContainerBuilder not created")
            .image("private-company/product:2.4.14")
            .resources(resources)
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
    fn builder_pod_name() {
        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("foo"))
            .build()
            .unwrap();

        assert_eq!(pod.metadata.name.unwrap(), "foo");
    }

    #[rstest]
    fn builder(pod_affinity: PodAffinity, dummy_container: Container) {
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
            .unwrap()
            .termination_grace_period(&Duration::from_secs(42))
            .unwrap()
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
                .and_then(|containers| containers.first().as_ref().map(|c| c.name.clone())),
            Some("init-containername".to_string())
        );

        assert_eq!(
            pod_spec.volumes.as_ref().and_then(|volumes| volumes
                .first()
                .as_ref()
                .and_then(|volume| volume.config_map.as_ref())
                .map(|cm| cm.name.clone())),
            Some("configmap".to_string())
        );
        assert_eq!(pod_spec.termination_grace_period_seconds, Some(42));
    }

    #[rstest]
    fn builder_image_pull_secrets(mut pod_builder_with_name_and_container: PodBuilder) {
        let pod = pod_builder_with_name_and_container
            .image_pull_secrets(vec!["company-registry-secret".to_string()].into_iter())
            .build()
            .unwrap();

        assert_eq!(
            pod.spec.unwrap().image_pull_secrets.unwrap(),
            vec![LocalObjectReference {
                name: "company-registry-secret".to_string()
            }]
        );
    }

    #[rstest]
    fn builder_restart_policy(mut pod_builder_with_name_and_container: PodBuilder) {
        let pod = pod_builder_with_name_and_container
            .restart_policy("Always")
            .build()
            .unwrap();
        assert_eq!(pod.spec.unwrap().restart_policy.unwrap(), "Always");
    }

    #[test]
    fn builder_too_long_termination_grace_period() {
        let too_long_duration = Duration::from_secs(i64::MAX as u64 + 1);
        let mut pod_builder = PodBuilder::new();

        let result = pod_builder.termination_grace_period(&too_long_duration);
        assert!(matches!(
            result,
            Err(Error::TerminationGracePeriodTooLong {
                source: TryFromIntError { .. },
                duration,
            }) if duration == too_long_duration
        ));
    }
}
