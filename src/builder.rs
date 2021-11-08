//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in defaults that make sense.
use crate::error::{Error, OperatorResult};
use crate::labels;
use chrono::Utc;
use k8s_openapi::api::core::v1::{
    ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, DownwardAPIVolumeSource,
    EmptyDirVolumeSource, EnvVar, Event, EventSource, HostPathVolumeSource, Node, ObjectReference,
    PersistentVolumeClaimVolumeSource, Pod, PodCondition, PodSecurityContext, PodSpec, PodStatus,
    PodTemplateSpec, Probe, ProjectedVolumeSource, SELinuxOptions, SeccompProfile,
    SecretVolumeSource, Sysctl, Toleration, Volume, VolumeMount, WindowsSecurityContextOptions,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, OwnerReference, Time};
use kube::{Resource, ResourceExt};
use std::collections::BTreeMap;
use tracing::warn;

/// A builder to build [`ConfigMap`] objects.
#[derive(Clone, Default)]
pub struct ConfigMapBuilder {
    metadata: Option<ObjectMeta>,
    data: Option<BTreeMap<String, String>>,
}

impl ConfigMapBuilder {
    pub fn new() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
    }

    pub fn metadata_default(&mut self) -> &mut Self {
        self.metadata(ObjectMeta::default());
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

    pub fn add_data(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.data
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }

    pub fn data(&mut self, data: BTreeMap<String, String>) -> &mut Self {
        self.data = Some(data);
        self
    }

    pub fn build(&self) -> OperatorResult<ConfigMap> {
        Ok(ConfigMap {
            metadata: match self.metadata {
                None => return Err(Error::MissingObjectKey { key: "metadata" }),
                Some(ref metadata) => metadata.clone(),
            },
            data: self.data.clone(),
            ..ConfigMap::default()
        })
    }
}

/// A builder to build [`Container`] objects.
///
/// This will automatically create the necessary volumes and mounts for each `ConfigMap` which is added.
#[derive(Clone, Default)]
pub struct ContainerBuilder {
    args: Option<Vec<String>>,
    container_ports: Option<Vec<ContainerPort>>,
    command: Option<Vec<String>>,
    env: Option<Vec<EnvVar>>,
    image: Option<String>,
    image_pull_policy: Option<String>,
    name: String,
    volume_mounts: Option<Vec<VolumeMount>>,
    readiness_probe: Option<Probe>,
    liveness_probe: Option<Probe>,
}

impl ContainerBuilder {
    pub fn new(name: &str) -> Self {
        ContainerBuilder {
            name: name.to_string(),
            ..ContainerBuilder::default()
        }
    }

    pub fn image(&mut self, image: impl Into<String>) -> &mut Self {
        self.image = Some(image.into());
        self
    }

    pub fn image_pull_policy(&mut self, image_pull_policy: impl Into<String>) -> &mut Self {
        self.image_pull_policy = Some(image_pull_policy.into());
        self
    }

    pub fn add_env_var(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.env.get_or_insert_with(Vec::new).push(EnvVar {
            name: name.into(),
            value: Some(value.into()),
            ..EnvVar::default()
        });
        self
    }

    pub fn add_env_vars(&mut self, env_vars: Vec<EnvVar>) -> &mut Self {
        self.env.get_or_insert_with(Vec::new).extend(env_vars);
        self
    }

    pub fn command(&mut self, command: Vec<String>) -> &mut Self {
        self.command = Some(command);
        self
    }

    pub fn args(&mut self, args: Vec<String>) -> &mut Self {
        self.args = Some(args);
        self
    }

    pub fn add_container_port(&mut self, name: impl Into<String>, port: i32) -> &mut Self {
        self.container_ports
            .get_or_insert_with(Vec::new)
            .push(ContainerPort {
                name: Some(name.into()),
                container_port: port,
                ..ContainerPort::default()
            });
        self
    }

    pub fn add_container_ports(&mut self, container_port: Vec<ContainerPort>) -> &mut Self {
        self.container_ports
            .get_or_insert_with(Vec::new)
            .extend(container_port);
        self
    }

    pub fn add_volume_mount(
        &mut self,
        name: impl Into<String>,
        path: impl Into<String>,
    ) -> &mut Self {
        self.volume_mounts
            .get_or_insert_with(Vec::new)
            .push(VolumeMount {
                name: name.into(),
                mount_path: path.into(),
                ..VolumeMount::default()
            });
        self
    }

    pub fn add_volume_mounts(
        &mut self,
        volume_mounts: impl IntoIterator<Item = VolumeMount>,
    ) -> &mut Self {
        self.volume_mounts
            .get_or_insert_with(Vec::new)
            .extend(volume_mounts);
        self
    }

    pub fn readiness_probe(&mut self, probe: Probe) -> &mut Self {
        self.readiness_probe = Some(probe);
        self
    }

    pub fn liveness_probe(&mut self, probe: Probe) -> &mut Self {
        self.liveness_probe = Some(probe);
        self
    }

    pub fn build(&self) -> Container {
        Container {
            args: self.args.clone(),
            command: self.command.clone(),
            env: self.env.clone(),
            image: self.image.clone(),
            image_pull_policy: self.image_pull_policy.clone(),
            name: self.name.clone(),
            ports: self.container_ports.clone(),
            volume_mounts: self.volume_mounts.clone(),
            readiness_probe: self.readiness_probe.clone(),
            liveness_probe: self.liveness_probe.clone(),
            ..Container::default()
        }
    }
}

/// A builder to build [`ContainerPort`] objects.
#[derive(Clone, Default)]
pub struct ContainerPortBuilder {
    container_port: i32,
    name: Option<String>,
    host_ip: Option<String>,
    protocol: Option<String>,
    host_port: Option<i32>,
}

impl ContainerPortBuilder {
    pub fn new(container_port: i32) -> Self {
        ContainerPortBuilder {
            container_port,
            ..ContainerPortBuilder::default()
        }
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn host_ip(&mut self, host_ip: impl Into<String>) -> &mut Self {
        self.host_ip = Some(host_ip.into());
        self
    }

    pub fn protocol(&mut self, protocol: impl Into<String>) -> &mut Self {
        self.protocol = Some(protocol.into());
        self
    }

    pub fn host_port(&mut self, host_port: i32) -> &mut Self {
        self.host_port = Some(host_port);
        self
    }

    pub fn build(&self) -> ContainerPort {
        ContainerPort {
            container_port: self.container_port,
            // container_port_names must be lowercase!
            name: self.name.clone().map(|s| s.to_lowercase()),
            host_ip: self.host_ip.clone(),
            protocol: self.protocol.clone(),
            host_port: self.host_port,
        }
    }
}

/// Type of Event.
/// The restriction to these two values is not hardcoded in Kubernetes but by convention only.
#[derive(Clone, Debug)]
pub enum EventType {
    Normal,
    Warning,
}

impl ToString for EventType {
    fn to_string(&self) -> String {
        match self {
            EventType::Normal => "Normal".to_string(),
            EventType::Warning => "Warning".to_string(),
        }
    }
}

/// A builder to build [`Event`] objects.
///
/// This is mainly useful for tests.
#[derive(Clone, Debug, Default)]
pub struct EventBuilder {
    name: String,
    involved_object: ObjectReference,
    event_type: Option<EventType>,
    action: Option<String>,
    reason: Option<String>,
    message: Option<String>,
    reporting_component: Option<String>,
    reporting_instance: Option<String>,
}

impl EventBuilder {
    /// Creates a new [`EventBuilder`].
    ///
    /// # Arguments
    ///
    /// - `resource` - The resource for which this event is created, will be used to create the `involvedObject` and `metadata.name` fields
    pub fn new<T>(resource: &T) -> EventBuilder
    where
        T: Resource<DynamicType = ()>,
    {
        let involved_object = ObjectReference {
            api_version: Some(T::api_version(&()).to_string()),
            field_path: None,
            kind: Some(T::kind(&()).to_string()),
            name: resource.meta().name.clone(),
            namespace: resource.namespace(),
            resource_version: resource.meta().resource_version.clone(),
            uid: resource.meta().uid.clone(),
        };

        EventBuilder {
            name: resource.name(),
            involved_object,
            ..EventBuilder::default()
        }
    }

    pub fn event_type(&mut self, event_type: &EventType) -> &mut Self {
        self.event_type = Some(event_type.clone());
        self
    }

    /// What action was taken/failed regarding to the Regarding object (e.g. Create, Update, Delete, Reconcile, ...)
    pub fn action(&mut self, action: impl Into<String>) -> &mut Self {
        self.action = Some(action.into());
        self
    }

    /// This should be a short, machine understandable string that gives the reason for this event being generated (e.g. PodMissing, UpdateRunning, ...)
    pub fn reason(&mut self, reason: impl Into<String>) -> &mut Self {
        self.reason = Some(reason.into());
        self
    }

    /// A human-readable description of the status of this operation.
    pub fn message(&mut self, message: impl Into<String>) -> &mut Self {
        self.message = Some(message.into());
        self
    }

    /// Name of the controller that emitted this Event, e.g. `kubernetes.io/kubelet`.
    pub fn reporting_component(&mut self, reporting_component: impl Into<String>) -> &mut Self {
        self.reporting_component = Some(reporting_component.into());
        self
    }

    /// ID of the controller instance, e.g. `kubelet-xyzf`.
    pub fn reporting_instance(&mut self, reporting_instance: impl Into<String>) -> &mut Self {
        self.reporting_instance = Some(reporting_instance.into());
        self
    }

    pub fn build(&self) -> Event {
        let time = Utc::now();

        let source = Some(EventSource {
            component: self.reporting_component.clone(),
            host: None,
        });

        Event {
            action: self.action.clone(),
            count: Some(1),
            event_time: Some(MicroTime(time)),
            first_timestamp: Some(Time(time)),
            involved_object: self.involved_object.clone(),
            last_timestamp: Some(Time(time)),
            message: self.message.clone(),
            metadata: ObjectMeta {
                generate_name: Some(format!("{}-", self.name)),
                ..ObjectMeta::default()
            },
            reason: self.reason.clone(),
            related: None,
            reporting_component: self.reporting_component.clone(),
            reporting_instance: self.reporting_instance.clone(),
            series: None,
            source,
            type_: self
                .event_type
                .as_ref()
                .map(|event_type| event_type.to_string()),
        }
    }
}

/// A builder to build [`Node`] objects.
///
/// This is mainly useful for tests.
#[derive(Default)]
pub struct NodeBuilder {
    node: Node,
}

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            node: Node::default(),
        }
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.node.metadata.name = Some(name.into());
        self
    }

    /// Consumes the Builder and returns a constructed Node
    pub fn build(&self) -> Node {
        // We're cloning here because we can't take just `self` in this method because then
        // we couldn't chain the method with the others easily (because they return &mut self and not self)
        self.node.clone()
    }
}

/// A builder to build [`ObjectMeta`] objects.
///
/// Of special interest is the [`Self::ownerreference_from_resource()`] function.
/// Note: This builder only supports a single `OwnerReference`.
///
/// It is strongly recommended to always call [`Self::with_recommended_labels()`]!
#[derive(Clone, Default)]
pub struct ObjectMetaBuilder {
    name: Option<String>,
    generate_name: Option<String>,
    namespace: Option<String>,
    ownerreference: Option<OwnerReference>,
    labels: Option<BTreeMap<String, String>>,
    annotations: Option<BTreeMap<String, String>>,
}

impl ObjectMetaBuilder {
    pub fn new() -> ObjectMetaBuilder {
        ObjectMetaBuilder::default()
    }

    /// This sets the name and namespace from a given resource
    pub fn name_and_namespace<T: Resource>(&mut self, resource: &T) -> &mut Self {
        self.name = Some(resource.name());
        self.namespace = resource.namespace();
        self
    }

    pub fn name_opt(&mut self, name: impl Into<Option<String>>) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn generate_name(&mut self, generate_name: impl Into<String>) -> &mut Self {
        self.generate_name = Some(generate_name.into());
        self
    }

    pub fn generate_name_opt(&mut self, generate_name: impl Into<Option<String>>) -> &mut Self {
        self.generate_name = generate_name.into();
        self
    }

    pub fn namespace_opt(&mut self, namespace: impl Into<Option<String>>) -> &mut Self {
        self.namespace = namespace.into();
        self
    }

    pub fn namespace(&mut self, namespace: impl Into<String>) -> &mut Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn ownerreference(&mut self, ownerreference: OwnerReference) -> &mut Self {
        self.ownerreference = Some(ownerreference);
        self
    }

    pub fn ownerreference_opt(&mut self, ownerreference: Option<OwnerReference>) -> &mut Self {
        self.ownerreference = ownerreference;
        self
    }

    /// This can be used to set the `OwnerReference` to the provided resource.
    pub fn ownerreference_from_resource<T: Resource<DynamicType = ()>>(
        &mut self,
        resource: &T,
        block_owner_deletion: Option<bool>,
        controller: Option<bool>,
    ) -> OperatorResult<&mut Self> {
        self.ownerreference = Some(
            OwnerReferenceBuilder::new()
                .initialize_from_resource(resource)
                .block_owner_deletion_opt(block_owner_deletion)
                .controller_opt(controller)
                .build()?,
        );
        Ok(self)
    }

    /// This adds a single annotation to the existing annotations.
    /// It'll override an annotation with the same key.
    pub fn with_annotation(
        &mut self,
        annotation_key: impl Into<String>,
        annotation_value: impl Into<String>,
    ) -> &mut Self {
        self.annotations
            .get_or_insert_with(BTreeMap::new)
            .insert(annotation_key.into(), annotation_value.into());
        self
    }

    /// This adds multiple annotations to the existing annotations.
    /// Any existing annotation with a key that is contained in `annotations` will be overwritten
    pub fn with_annotations(&mut self, annotations: BTreeMap<String, String>) -> &mut Self {
        self.annotations
            .get_or_insert_with(BTreeMap::new)
            .extend(annotations);
        self
    }

    /// This will replace all existing annotations
    pub fn annotations(&mut self, annotations: BTreeMap<String, String>) -> &mut Self {
        self.annotations = Some(annotations);
        self
    }

    /// This adds a single label to the existing labels.
    /// It'll override a label with the same key.
    pub fn with_label(
        &mut self,
        label_key: impl Into<String>,
        label_value: impl Into<String>,
    ) -> &mut Self {
        self.labels
            .get_or_insert_with(BTreeMap::new)
            .insert(label_key.into(), label_value.into());
        self
    }

    /// This adds multiple labels to the existing labels.
    /// Any existing label with a key that is contained in `labels` will be overwritten
    pub fn with_labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.labels.get_or_insert_with(BTreeMap::new).extend(labels);
        self
    }

    /// This will replace all existing labels
    pub fn labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.labels = Some(labels);
        self
    }

    /// This sets the common recommended labels (in the `app.kubernetes.io` namespace).
    /// It is recommended to always call this method.
    /// The only reasons it is not _required_ is to make testing easier and to allow for more
    /// flexibility if needed.
    pub fn with_recommended_labels<T: Resource>(
        &mut self,
        resource: &T,
        app_name: &str,
        app_version: &str,
        app_component: &str,
        role_name: &str,
    ) -> &mut Self {
        let recommended_labels = labels::get_recommended_labels(
            resource,
            app_name,
            app_version,
            app_component,
            role_name,
        );
        self.labels
            .get_or_insert_with(BTreeMap::new)
            .extend(recommended_labels);
        self
    }

    pub fn build(&self) -> ObjectMeta {
        // if 'generate_name' and 'name' are set, Kubernetes will prioritize the 'name' field and
        // 'generate_name' has no impact.
        if let (Some(name), Some(generate_name)) = (&self.name, &self.generate_name) {
            warn!(
                "ObjectMeta has a 'name' [{}] and 'generate_name' [{}] field set. Kubernetes \
		 will prioritize the 'name' field over 'generate_name'.",
                name, generate_name
            );
        }

        ObjectMeta {
            generate_name: self.generate_name.clone(),
            name: self.name.clone(),
            namespace: self.namespace.clone(),
            owner_references: self
                .ownerreference
                .as_ref()
                .map(|ownerreference| vec![ownerreference.clone()]),
            labels: self.labels.clone(),
            annotations: self.annotations.clone(),
            ..ObjectMeta::default()
        }
    }
}

/// A builder to build [`OwnerReference`] objects.
///
/// Of special interest is the [`Self::initialize_from_resource()`] function.
#[derive(Clone, Default)]
pub struct OwnerReferenceBuilder {
    api_version: Option<String>,
    block_owner_deletion: Option<bool>,
    controller: Option<bool>,
    kind: Option<String>,
    name: Option<String>,
    uid: Option<String>,
}

impl OwnerReferenceBuilder {
    pub fn new() -> OwnerReferenceBuilder {
        OwnerReferenceBuilder::default()
    }

    pub fn api_version(&mut self, api_version: impl Into<String>) -> &mut Self {
        self.api_version = Some(api_version.into());
        self
    }

    pub fn api_version_opt(&mut self, api_version: impl Into<Option<String>>) -> &mut Self {
        self.api_version = api_version.into();
        self
    }

    pub fn block_owner_deletion(&mut self, block_owner_deletion: bool) -> &mut Self {
        self.block_owner_deletion = Some(block_owner_deletion);
        self
    }

    pub fn block_owner_deletion_opt(&mut self, block_owner_deletion: Option<bool>) -> &mut Self {
        self.block_owner_deletion = block_owner_deletion;
        self
    }

    pub fn controller(&mut self, controller: bool) -> &mut Self {
        self.controller = Some(controller);
        self
    }

    pub fn controller_opt(&mut self, controller: Option<bool>) -> &mut Self {
        self.controller = controller;
        self
    }

    pub fn kind(&mut self, kind: impl Into<String>) -> &mut Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn kind_opt(&mut self, kind: impl Into<Option<String>>) -> &mut Self {
        self.kind = kind.into();
        self
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn name_opt(&mut self, name: impl Into<Option<String>>) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn uid(&mut self, uid: impl Into<String>) -> &mut Self {
        self.uid = Some(uid.into());
        self
    }

    pub fn uid_opt(&mut self, uid: impl Into<Option<String>>) -> &mut Self {
        self.uid = uid.into();
        self
    }

    /// Can be used to initialize a builder with settings from an existing resource.
    /// The builder will create an `OwnerReference` that points to the passed resource.
    ///
    /// This will _not_ set `controller` or `block_owner_deletion`.
    pub fn initialize_from_resource<T: Resource<DynamicType = ()>>(
        &mut self,
        resource: &T,
    ) -> &mut Self {
        self.api_version(T::api_version(&()))
            .kind(T::kind(&()))
            .name(resource.name())
            .uid_opt(resource.meta().uid.clone());
        self
    }

    pub fn build(&self) -> OperatorResult<OwnerReference> {
        Ok(OwnerReference {
            api_version: match self.api_version {
                None => return Err(Error::MissingObjectKey { key: "api_version" }),
                Some(ref api_version) => api_version.clone(),
            },
            block_owner_deletion: self.block_owner_deletion,
            controller: self.controller,
            kind: match self.kind {
                None => return Err(Error::MissingObjectKey { key: "kind" }),
                Some(ref kind) => kind.clone(),
            },
            name: match self.name {
                None => return Err(Error::MissingObjectKey { key: "name" }),
                Some(ref name) => name.clone(),
            },
            uid: match self.uid {
                None => return Err(Error::MissingObjectKey { key: "uid" }),
                Some(ref uid) => uid.clone(),
            },
        })
    }
}

#[derive(Clone, Default)]
pub struct PodSecurityContextBuilder {
    pod_security_context: PodSecurityContext,
}

impl PodSecurityContextBuilder {
    pub fn new() -> PodSecurityContextBuilder {
        PodSecurityContextBuilder::default()
    }

    pub fn build(&self) -> PodSecurityContext {
        self.pod_security_context.clone()
    }

    pub fn fs_group(&mut self, group: i64) -> &mut Self {
        self.pod_security_context.fs_group = Some(group);
        self
    }

    pub fn fs_group_change_policy(&mut self, policy: &str) -> &mut Self {
        self.pod_security_context.fs_group_change_policy = Some(policy.to_string());
        self
    }

    pub fn run_as_group(&mut self, group: i64) -> &mut Self {
        self.pod_security_context.run_as_group = Some(group);
        self
    }

    pub fn run_as_non_root(&mut self) -> &mut Self {
        self.pod_security_context.run_as_non_root = Some(true);
        self
    }

    pub fn run_as_user(&mut self, user: i64) -> &mut Self {
        self.pod_security_context.run_as_user = Some(user);
        self
    }

    pub fn supplemental_groups(&mut self, groups: &[i64]) -> &mut Self {
        self.pod_security_context.supplemental_groups = Some(groups.to_vec());
        self
    }

    pub fn se_linux_level(&mut self, level: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    level: Some(level.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    level: Some(level.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_role(&mut self, role: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    role: Some(role.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    role: Some(role.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_type(&mut self, type_: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    type_: Some(type_.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    type_: Some(type_.to_string()),
                    ..o
                },
            ));
        self
    }
    pub fn se_linux_user(&mut self, user: &str) -> &mut Self {
        self.pod_security_context.se_linux_options =
            Some(self.pod_security_context.se_linux_options.clone().map_or(
                SELinuxOptions {
                    user: Some(user.to_string()),
                    ..SELinuxOptions::default()
                },
                |o| SELinuxOptions {
                    user: Some(user.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn seccomp_profile_localhost(&mut self, profile: &str) -> &mut Self {
        self.pod_security_context.seccomp_profile =
            Some(self.pod_security_context.seccomp_profile.clone().map_or(
                SeccompProfile {
                    localhost_profile: Some(profile.to_string()),
                    ..SeccompProfile::default()
                },
                |o| SeccompProfile {
                    localhost_profile: Some(profile.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn seccomp_profile_type(&mut self, type_: &str) -> &mut Self {
        self.pod_security_context.seccomp_profile =
            Some(self.pod_security_context.seccomp_profile.clone().map_or(
                SeccompProfile {
                    type_: type_.to_string(),
                    ..SeccompProfile::default()
                },
                |o| SeccompProfile {
                    type_: type_.to_string(),
                    ..o
                },
            ));
        self
    }

    pub fn sysctls(&mut self, kparam: &[(&str, &str)]) -> &mut Self {
        self.pod_security_context.sysctls = Some(
            kparam
                .iter()
                .map(|&name_value| Sysctl {
                    name: name_value.0.to_string(),
                    value: name_value.1.to_string(),
                })
                .collect(),
        );
        self
    }

    pub fn win_credential_spec(&mut self, spec: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some(spec.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some(spec.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn win_credential_spec_name(&mut self, name: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    gmsa_credential_spec_name: Some(name.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    gmsa_credential_spec_name: Some(name.to_string()),
                    ..o
                },
            ));
        self
    }

    pub fn win_run_as_user_name(&mut self, name: &str) -> &mut Self {
        self.pod_security_context.windows_options =
            Some(self.pod_security_context.windows_options.clone().map_or(
                WindowsSecurityContextOptions {
                    run_as_user_name: Some(name.to_string()),
                    ..WindowsSecurityContextOptions::default()
                },
                |o| WindowsSecurityContextOptions {
                    run_as_user_name: Some(name.to_string()),
                    ..o
                },
            ));
        self
    }
}

/// A builder to build [`Pod`] objects.
///
#[derive(Clone, Default)]
pub struct PodBuilder {
    containers: Vec<Container>,
    host_network: Option<bool>,
    init_containers: Option<Vec<Container>>,
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,
    status: Option<PodStatus>,
    security_context: Option<PodSecurityContext>,
    tolerations: Option<Vec<Toleration>>,
    volumes: Option<Vec<Volume>>,
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        PodBuilder::default()
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

    fn build_spec(&self) -> PodSpec {
        PodSpec {
            containers: self.containers.clone(),
            host_network: self.host_network,
            init_containers: self.init_containers.clone(),
            node_name: self.node_name.clone(),
            security_context: self.security_context.clone(),
            tolerations: self.tolerations.clone(),
            volumes: self.volumes.clone(),
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

    /// Returns a [`PodTemplateSpec`], usable for building a [`StatefulSet`] or [`Deployment`]
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

/// A builder to build [`Volume`] objects.
/// May only contain one `volume_source` at a time.
/// E.g. a call like `secret` after `empty_dir` will overwrite the `empty_dir`.
#[derive(Clone, Default)]
pub struct VolumeBuilder {
    name: String,
    volume_source: VolumeSource,
}

#[derive(Clone)]
pub enum VolumeSource {
    ConfigMap(ConfigMapVolumeSource),
    DownwardApi(DownwardAPIVolumeSource),
    EmptyDir(EmptyDirVolumeSource),
    HostPath(HostPathVolumeSource),
    PersistentVolumeClaim(PersistentVolumeClaimVolumeSource),
    Projected(ProjectedVolumeSource),
    Secret(SecretVolumeSource),
}

impl Default for VolumeSource {
    fn default() -> Self {
        Self::EmptyDir(EmptyDirVolumeSource {
            ..EmptyDirVolumeSource::default()
        })
    }
}

impl VolumeBuilder {
    pub fn new(name: impl Into<String>) -> VolumeBuilder {
        VolumeBuilder {
            name: name.into(),
            ..VolumeBuilder::default()
        }
    }

    pub fn config_map(&mut self, config_map: impl Into<ConfigMapVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::ConfigMap(config_map.into());
        self
    }

    pub fn with_config_map(&mut self, name: impl Into<String>) -> &mut Self {
        self.volume_source = VolumeSource::ConfigMap(ConfigMapVolumeSource {
            name: Some(name.into()),
            ..ConfigMapVolumeSource::default()
        });
        self
    }

    pub fn downward_api(&mut self, downward_api: impl Into<DownwardAPIVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::DownwardApi(downward_api.into());
        self
    }

    pub fn empty_dir(&mut self, empty_dir: impl Into<EmptyDirVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::EmptyDir(empty_dir.into());
        self
    }

    pub fn with_empty_dir(
        &mut self,
        medium: Option<impl Into<String>>,
        quantity: Option<Quantity>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::EmptyDir(EmptyDirVolumeSource {
            medium: medium.map(|m| m.into()),
            size_limit: quantity,
        });
        self
    }

    pub fn host_path(&mut self, host_path: impl Into<HostPathVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::HostPath(host_path.into());
        self
    }

    pub fn with_host_path(
        &mut self,
        path: impl Into<String>,
        type_: Option<impl Into<String>>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::HostPath(HostPathVolumeSource {
            path: path.into(),
            type_: type_.map(|t| t.into()),
        });
        self
    }

    pub fn persistent_volume_claim(
        &mut self,
        persistent_volume_claim: impl Into<PersistentVolumeClaimVolumeSource>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::PersistentVolumeClaim(persistent_volume_claim.into());
        self
    }

    pub fn with_persistent_volume_claim(
        &mut self,
        claim_name: impl Into<String>,
        read_only: bool,
    ) -> &mut Self {
        self.volume_source =
            VolumeSource::PersistentVolumeClaim(PersistentVolumeClaimVolumeSource {
                claim_name: claim_name.into(),
                read_only: Some(read_only),
            });
        self
    }

    pub fn projected(&mut self, projected: impl Into<ProjectedVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Projected(projected.into());
        self
    }

    pub fn secret(&mut self, secret: impl Into<SecretVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Secret(secret.into());
        self
    }

    pub fn with_secret(&mut self, secret_name: impl Into<String>, optional: bool) -> &mut Self {
        self.volume_source = VolumeSource::Secret(SecretVolumeSource {
            optional: Some(optional),
            secret_name: Some(secret_name.into()),
            ..SecretVolumeSource::default()
        });
        self
    }

    /// Consumes the Builder and returns a constructed Volume
    pub fn build(&self) -> Volume {
        Volume {
            name: self.name.clone(),
            config_map: match &self.volume_source {
                VolumeSource::ConfigMap(configmap) => Some(configmap.clone()),
                _ => None,
            },
            downward_api: match &self.volume_source {
                VolumeSource::DownwardApi(downward_api) => Some(downward_api.clone()),
                _ => None,
            },
            empty_dir: match &self.volume_source {
                VolumeSource::EmptyDir(empty_dir) => Some(empty_dir.clone()),
                _ => None,
            },
            host_path: match &self.volume_source {
                VolumeSource::HostPath(host_path) => Some(host_path.clone()),
                _ => None,
            },
            persistent_volume_claim: match &self.volume_source {
                VolumeSource::PersistentVolumeClaim(pvc) => Some(pvc.clone()),
                _ => None,
            },
            projected: match &self.volume_source {
                VolumeSource::Projected(projected) => Some(projected.clone()),
                _ => None,
            },
            secret: match &self.volume_source {
                VolumeSource::Secret(secret) => Some(secret.clone()),
                _ => None,
            },
            ..Volume::default()
        }
    }
}

/// A builder to build [`VolumeMount`] objects.
///
#[derive(Clone, Default)]
pub struct VolumeMountBuilder {
    mount_path: String,
    mount_propagation: Option<String>,
    name: String,
    read_only: Option<bool>,
    sub_path: Option<String>,
    sub_path_expr: Option<String>,
}

impl VolumeMountBuilder {
    pub fn new(name: impl Into<String>, mount_path: impl Into<String>) -> VolumeMountBuilder {
        VolumeMountBuilder {
            mount_path: mount_path.into(),
            name: name.into(),
            ..VolumeMountBuilder::default()
        }
    }

    pub fn read_only(&mut self, read_only: bool) -> &mut Self {
        self.read_only = Some(read_only);
        self
    }

    pub fn mount_propagation(&mut self, mount_propagation: impl Into<String>) -> &mut Self {
        self.mount_propagation = Some(mount_propagation.into());
        self
    }

    pub fn sub_path(&mut self, sub_path: impl Into<String>) -> &mut Self {
        self.sub_path = Some(sub_path.into());
        self
    }

    pub fn sub_path_expr(&mut self, sub_path_expr: impl Into<String>) -> &mut Self {
        self.sub_path_expr = Some(sub_path_expr.into());
        self
    }

    /// Consumes the Builder and returns a constructed VolumeMount
    pub fn build(&self) -> VolumeMount {
        VolumeMount {
            mount_path: self.mount_path.clone(),
            mount_propagation: self.mount_propagation.clone(),
            name: self.name.clone(),
            read_only: self.read_only,
            sub_path: self.sub_path.clone(),
            sub_path_expr: self.sub_path_expr.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{
        ConfigMapBuilder, ContainerBuilder, ContainerPortBuilder, EventBuilder, EventType,
        NodeBuilder, ObjectMetaBuilder, PodBuilder, PodSecurityContextBuilder, VolumeBuilder,
        VolumeMountBuilder,
    };
    use k8s_openapi::api::core::v1::{
        EnvVar, Pod, PodSecurityContext, SELinuxOptions, SeccompProfile, Sysctl, VolumeMount,
        WindowsSecurityContextOptions,
    };
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use std::collections::BTreeMap;

    #[test]
    fn test_security_context_builder() {
        let mut builder = PodSecurityContextBuilder::new();
        let context = builder
            .fs_group(1000)
            .fs_group_change_policy("policy")
            .run_as_user(1001)
            .run_as_group(1001)
            .run_as_non_root()
            .supplemental_groups(&[1002, 1003])
            .se_linux_level("level")
            .se_linux_role("role")
            .se_linux_type("type")
            .se_linux_user("user")
            .seccomp_profile_localhost("localhost")
            .seccomp_profile_type("type")
            .sysctls(&[("param1", "value1"), ("param2", "value2")])
            .win_credential_spec("spec")
            .win_credential_spec_name("name")
            .win_run_as_user_name("winuser")
            .build();

        assert_eq!(
            context,
            PodSecurityContext {
                fs_group: Some(1000),
                fs_group_change_policy: Some("policy".to_string()),
                run_as_user: Some(1001),
                run_as_group: Some(1001),
                run_as_non_root: Some(true),
                supplemental_groups: Some(vec![1002, 1003]),
                se_linux_options: Some(SELinuxOptions {
                    level: Some("level".to_string()),
                    role: Some("role".to_string()),
                    type_: Some("type".to_string()),
                    user: Some("user".to_string()),
                }),
                seccomp_profile: Some(SeccompProfile {
                    localhost_profile: Some("localhost".to_string()),
                    type_: "type".to_string(),
                }),
                sysctls: Some(vec![
                    Sysctl {
                        name: "param1".to_string(),
                        value: "value1".to_string(),
                    },
                    Sysctl {
                        name: "param2".to_string(),
                        value: "value2".to_string(),
                    },
                ]),
                windows_options: Some(WindowsSecurityContextOptions {
                    gmsa_credential_spec: Some("spec".to_string()),
                    gmsa_credential_spec_name: Some("name".to_string()),
                    run_as_user_name: Some("winuser".to_string()),
                    ..Default::default()
                })
            }
        );
    }

    #[test]
    fn test_configmap_builder() {
        let mut data = BTreeMap::new();
        data.insert("foo".to_string(), "bar".to_string());
        let configmap = ConfigMapBuilder::new()
            .data(data)
            .add_data("bar", "foo")
            .metadata_opt(Some(ObjectMetaBuilder::new().name("test").build()))
            .build()
            .unwrap();

        assert!(matches!(configmap.data.as_ref().unwrap().get("foo"), Some(bar) if bar == "bar"));
        assert!(matches!(configmap.data.as_ref().unwrap().get("bar"), Some(bar) if bar == "foo"));
    }

    #[test]
    fn test_container_builder() {
        let container_port: i32 = 10000;
        let container_port_name = "foo_port_name";
        let container_port_1: i32 = 20000;
        let container_port_name_1 = "bar_port_name";

        let container = ContainerBuilder::new("testcontainer")
            .add_env_var("foo", "bar")
            .add_volume_mount("configmap", "/mount")
            .add_container_port(container_port_name, container_port)
            .add_container_ports(vec![ContainerPortBuilder::new(container_port_1)
                .name(container_port_name_1)
                .build()])
            .build();

        assert_eq!(container.name, "testcontainer");
        assert!(
            matches!(container.env.unwrap().get(0), Some(EnvVar {name, value: Some(value), ..}) if name == "foo" && value == "bar")
        );
        assert_eq!(container.volume_mounts.as_ref().unwrap().len(), 1);
        assert!(
            matches!(container.volume_mounts.as_ref().unwrap().get(0), Some(VolumeMount {mount_path, name, ..}) if mount_path == "/mount" && name == "configmap")
        );
        assert_eq!(container.ports.as_ref().unwrap().len(), 2);
        assert_eq!(
            container
                .ports
                .as_ref()
                .map(|ports| (&ports[0].name, ports[0].container_port)),
            Some((&Some(container_port_name.to_string()), container_port))
        );
        assert_eq!(
            container
                .ports
                .as_ref()
                .map(|ports| (&ports[1].name, ports[1].container_port)),
            Some((&Some(container_port_name_1.to_string()), container_port_1))
        );
    }

    #[test]
    fn test_container_port_builder() {
        let port: i32 = 10000;
        let name = "FooBar";
        let protocol = "http";
        let host_port = 20000;
        let host_ip = "1.1.1.1";
        let container_port = ContainerPortBuilder::new(port)
            .name(name)
            .protocol(protocol)
            .host_port(host_port)
            .host_ip(host_ip)
            .build();

        assert_eq!(container_port.container_port, port);
        assert_eq!(container_port.name, Some(name.to_lowercase()));
        assert_eq!(container_port.protocol, Some(protocol.to_string()));
        assert_eq!(container_port.host_ip, Some(host_ip.to_string()));
        assert_eq!(container_port.host_port, Some(host_port));
    }

    #[test]
    fn test_event_builder() {
        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("testpod"))
            .build()
            .unwrap();

        let event = EventBuilder::new(&pod)
            .event_type(&EventType::Normal)
            .action("action")
            .message("message")
            .build();

        assert!(matches!(event.involved_object.kind, Some(pod_name) if pod_name == "Pod"));

        assert!(matches!(event.message, Some(message) if message == "message"));
        assert!(matches!(event.reason, None));
    }

    #[test]
    fn test_node_builder() {
        let node = NodeBuilder::new().name("node").build();

        assert!(matches!(node.metadata.name, Some(name) if name == "node"));
    }

    #[test]
    fn test_objectmeta_builder() {
        let mut pod = Pod::default();
        pod.metadata.name = Some("pod".to_string());
        pod.metadata.uid = Some("uid".to_string());

        let meta = ObjectMetaBuilder::new()
            .generate_name("generate_foo")
            .name("foo")
            .namespace("bar")
            .ownerreference_from_resource(&pod, Some(true), Some(false))
            .unwrap()
            .with_recommended_labels(&pod, "test_app", "1.0", "component", "role")
            .with_annotation("foo", "bar")
            .build();

        assert_eq!(meta.generate_name, Some("generate_foo".to_string()));
        assert_eq!(meta.name, Some("foo".to_string()));
        assert_eq!(meta.owner_references.as_ref().unwrap().len(), 1);
        assert!(
            matches!(meta.owner_references.unwrap().get(0), Some(OwnerReference { uid, ..}) if uid == "uid")
        );
        assert_eq!(meta.annotations.as_ref().unwrap().len(), 1);
        assert_eq!(
            meta.annotations.as_ref().unwrap().get(&"foo".to_string()),
            Some(&"bar".to_string())
        );
    }

    #[test]
    fn test_volume_mount_builder() {
        let mut volume_mount_builder = VolumeMountBuilder::new("name", "mount_path");
        volume_mount_builder
            .mount_propagation("mount_propagation")
            .read_only(true)
            .sub_path("sub_path")
            .sub_path_expr("sub_path_expr");

        let vm = volume_mount_builder.build();

        assert_eq!(vm.name, "name".to_string());
        assert_eq!(vm.mount_path, "mount_path".to_string());
        assert_eq!(vm.mount_propagation, Some("mount_propagation".to_string()));
        assert_eq!(vm.read_only, Some(true));
        assert_eq!(vm.sub_path, Some("sub_path".to_string()));
        assert_eq!(vm.sub_path_expr, Some("sub_path_expr".to_string()));
    }

    #[test]
    fn test_volume_builder() {
        let mut volume_builder = VolumeBuilder::new("name");
        volume_builder.with_config_map("configmap");
        let vol = volume_builder.build();

        assert_eq!(vol.name, "name".to_string());
        assert_eq!(
            vol.config_map.and_then(|cm| cm.name),
            Some("configmap".to_string())
        );

        volume_builder.with_empty_dir(Some("medium"), Some(Quantity("quantity".to_string())));
        let vol = volume_builder.build();

        assert_eq!(
            vol.empty_dir.and_then(|dir| dir.medium),
            Some("medium".to_string())
        );

        volume_builder.with_host_path("path", Some("type_"));
        let vol = volume_builder.build();

        assert_eq!(
            vol.host_path.map(|host| host.path),
            Some("path".to_string())
        );

        volume_builder.with_secret("secret", false);
        let vol = volume_builder.build();

        assert_eq!(
            vol.secret.and_then(|secret| secret.secret_name),
            Some("secret".to_string())
        );
    }

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

        let pod = PodBuilder::new()
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
}
