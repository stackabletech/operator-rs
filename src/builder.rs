//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in defaults that make sense.
use crate::error::{Error, OperatorResult};
use crate::labels;
use chrono::Utc;
use k8s_openapi::api::core::v1::{
    ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, EnvVar, Event, EventSource, Node,
    ObjectReference, Pod, PodCondition, PodSpec, PodStatus, Toleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, OwnerReference, Time};
use kube::{Resource, ResourceExt};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A builder to build [`ConfigMap`] objects.
#[derive(Clone, Default)]
pub struct ConfigMapBuilder {
    metadata: Option<ObjectMeta>,
    data: BTreeMap<String, String>,
}

impl ConfigMapBuilder {
    pub fn new() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
    }

    pub fn metadata_default(&mut self) -> &mut Self {
        self.metadata(ObjectMeta::default());
        self
    }

    pub fn metadata<VALUE: Into<ObjectMeta>>(&mut self, metadata: VALUE) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn metadata_opt<VALUE: Into<Option<ObjectMeta>>>(&mut self, metadata: VALUE) -> &mut Self {
        self.metadata = metadata.into();
        self
    }

    pub fn add_data<KEY: Into<String>, VALUE: Into<String>>(
        &mut self,
        key: KEY,
        value: VALUE,
    ) -> &mut Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn data(&mut self, data: BTreeMap<String, String>) -> &mut Self {
        self.data = data;
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
    image: Option<String>,
    name: String,
    env: Vec<EnvVar>,
    command: Vec<String>,
    args: Vec<String>,
    configmaps: HashMap<String, String>,
    container_ports: Vec<ContainerPort>,
}

impl ContainerBuilder {
    pub fn new(name: &str) -> Self {
        ContainerBuilder {
            name: name.to_string(),
            ..ContainerBuilder::default()
        }
    }

    pub fn image<VALUE: Into<String>>(&mut self, image: VALUE) -> &mut Self {
        self.image = Some(image.into());
        self
    }

    pub fn add_env_var<NAME: Into<String>, VALUE: Into<String>>(
        &mut self,
        name: NAME,
        value: VALUE,
    ) -> &mut Self {
        self.env.push(EnvVar {
            name: name.into(),
            value: Some(value.into()),
            ..EnvVar::default()
        });
        self
    }

    pub fn add_env_vars(&mut self, env_vars: Vec<EnvVar>) -> &mut Self {
        self.env.extend(env_vars);
        self
    }

    pub fn command(&mut self, command: Vec<String>) -> &mut Self {
        self.command = command;
        self
    }

    pub fn args(&mut self, args: Vec<String>) -> &mut Self {
        self.args = args;
        self
    }

    /// This adds a [`VolumeMount`] and [`ConfigMapVolumeSource`] to the current container.
    ///
    /// This method does not do any validation on the name of the `ConfigMap` or the mount path.
    pub fn add_configmapvolume<NAME: Into<String>, PATH: Into<String>>(
        &mut self,
        configmap_name: NAME,
        mount_path: PATH,
    ) -> &mut Self {
        self.configmaps
            .insert(mount_path.into(), configmap_name.into());
        self
    }

    pub fn add_container_port(&mut self, container_port: ContainerPort) -> &mut Self {
        self.container_ports.push(container_port);
        self
    }

    pub fn add_container_ports(&mut self, container_port: Vec<ContainerPort>) -> &mut Self {
        self.container_ports.extend(container_port);
        self
    }

    pub fn build(&self) -> Container {
        let mut volumes = vec![];
        let mut volume_mounts = vec![];
        for (mount_path, configmap_name) in &self.configmaps {
            let volume = Volume {
                name: configmap_name.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(configmap_name.clone()),
                    ..ConfigMapVolumeSource::default()
                }),
                ..Volume::default()
            };
            volumes.push(volume);

            let volume_mount = VolumeMount {
                name: configmap_name.clone(),
                mount_path: mount_path.clone(),
                ..VolumeMount::default()
            };
            volume_mounts.push(volume_mount);
        }

        Container {
            image: self.image.clone(),
            name: self.name.clone(),
            env: self.env.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            volume_mounts,
            ports: self.container_ports.clone(),
            ..Container::default()
        }
    }
}

/// A builder to build [`ContainerPort`] objects.
#[derive(Clone, Default)]
pub struct ContainerPortBuilder {
    container_port: u16,
    name: Option<String>,
    host_ip: Option<String>,
    protocol: Option<String>,
    host_port: Option<u16>,
}

impl ContainerPortBuilder {
    pub fn new(container_port: u16) -> Self {
        ContainerPortBuilder {
            container_port,
            ..ContainerPortBuilder::default()
        }
    }

    pub fn name<VALUE: Into<String>>(&mut self, name: VALUE) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn host_ip<VALUE: Into<String>>(&mut self, host_ip: VALUE) -> &mut Self {
        self.host_ip = Some(host_ip.into());
        self
    }

    pub fn protocol<VALUE: Into<String>>(&mut self, protocol: VALUE) -> &mut Self {
        self.protocol = Some(protocol.into());
        self
    }

    pub fn host_port(&mut self, host_port: u16) -> &mut Self {
        self.host_port = Some(host_port);
        self
    }

    pub fn build(&self) -> ContainerPort {
        ContainerPort {
            container_port: i32::from(self.container_port),
            // container_port_names must be lowercase!
            name: self.name.clone().map(|s| s.to_lowercase()),
            host_ip: self.host_ip.clone(),
            protocol: self.protocol.clone(),
            host_port: self.host_port.map(i32::from),
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
    pub fn action<VALUE: Into<String>>(&mut self, action: VALUE) -> &mut Self {
        self.action = Some(action.into());
        self
    }

    /// This should be a short, machine understandable string that gives the reason for this event being generated (e.g. PodMissing, UpdateRunning, ...)
    pub fn reason<VALUE: Into<String>>(&mut self, reason: VALUE) -> &mut Self {
        self.reason = Some(reason.into());
        self
    }

    /// A human-readable description of the status of this operation.
    pub fn message<VALUE: Into<String>>(&mut self, message: VALUE) -> &mut Self {
        self.message = Some(message.into());
        self
    }

    /// Name of the controller that emitted this Event, e.g. `kubernetes.io/kubelet`.
    pub fn reporting_component<VALUE: Into<String>>(
        &mut self,
        reporting_component: VALUE,
    ) -> &mut Self {
        self.reporting_component = Some(reporting_component.into());
        self
    }

    /// ID of the controller instance, e.g. `kubelet-xyzf`.
    pub fn reporting_instance<VALUE: Into<String>>(
        &mut self,
        reporting_instance: VALUE,
    ) -> &mut Self {
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

    pub fn name<VALUE: Into<String>>(&mut self, name: VALUE) -> &mut Self {
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
    labels: BTreeMap<String, String>,
    annotations: BTreeMap<String, String>,
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

    pub fn name_opt<VALUE: Into<Option<String>>>(&mut self, name: VALUE) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn name<VALUE: Into<String>>(&mut self, name: VALUE) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn generate_name<VALUE: Into<String>>(&mut self, generate_name: VALUE) -> &mut Self {
        self.generate_name = Some(generate_name.into());
        self
    }

    pub fn generate_name_opt<VALUE: Into<Option<String>>>(
        &mut self,
        generate_name: VALUE,
    ) -> &mut Self {
        self.generate_name = generate_name.into();
        self
    }

    pub fn namespace_opt<VALUE: Into<Option<String>>>(&mut self, namespace: VALUE) -> &mut Self {
        self.namespace = namespace.into();
        self
    }

    pub fn namespace<VALUE: Into<String>>(&mut self, namespace: VALUE) -> &mut Self {
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
    pub fn with_annotation<KEY, VALUE>(
        &mut self,
        annotation_key: KEY,
        annotation_value: VALUE,
    ) -> &mut Self
    where
        KEY: Into<String>,
        VALUE: Into<String>,
    {
        self.annotations
            .insert(annotation_key.into(), annotation_value.into());
        self
    }

    /// This adds multiple annotations to the existing annotations.
    /// Any existing annotation with a key that is contained in `annotations` will be overwritten
    pub fn with_annotations(&mut self, annotations: BTreeMap<String, String>) -> &mut Self {
        self.annotations.extend(annotations);
        self
    }

    /// This will replace all existing annotations
    pub fn annotations(&mut self, annotations: BTreeMap<String, String>) -> &mut Self {
        self.annotations = annotations;
        self
    }

    /// This adds a single label to the existing labels.
    /// It'll override a label with the same key.
    pub fn with_label<KEY, VALUE>(&mut self, label_key: KEY, label_value: VALUE) -> &mut Self
    where
        KEY: Into<String>,
        VALUE: Into<String>,
    {
        self.labels.insert(label_key.into(), label_value.into());
        self
    }

    /// This adds multiple labels to the existing labels.
    /// Any existing label with a key that is contained in `labels` will be overwritten
    pub fn with_labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.labels.extend(labels);
        self
    }

    /// This will replace all existing labels
    pub fn labels(&mut self, labels: BTreeMap<String, String>) -> &mut Self {
        self.labels = labels;
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
        self.labels.extend(recommended_labels);
        self
    }

    pub fn build(&self) -> OperatorResult<ObjectMeta> {
        Ok(ObjectMeta {
            generate_name: self.generate_name.clone(),
            name: self.name.clone(),
            namespace: self.namespace.clone(),
            owner_references: match self.ownerreference {
                Some(ref ownerreference) => vec![ownerreference.clone()],
                None => vec![],
            },
            labels: self.labels.clone(),
            annotations: self.annotations.clone(),
            ..ObjectMeta::default()
        })
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

    pub fn api_version<VALUE: Into<String>>(&mut self, api_version: VALUE) -> &mut Self {
        self.api_version = Some(api_version.into());
        self
    }

    pub fn api_version_opt<VALUE: Into<Option<String>>>(
        &mut self,
        api_version: VALUE,
    ) -> &mut Self {
        self.api_version = api_version.into();
        self
    }

    pub fn block_owner_deletion<VALUE: Into<bool>>(
        &mut self,
        block_owner_deletion: VALUE,
    ) -> &mut Self {
        self.block_owner_deletion = Some(block_owner_deletion.into());
        self
    }

    pub fn block_owner_deletion_opt<VALUE: Into<Option<bool>>>(
        &mut self,
        block_owner_deletion: VALUE,
    ) -> &mut Self {
        self.block_owner_deletion = block_owner_deletion.into();
        self
    }

    pub fn controller<VALUE: Into<bool>>(&mut self, controller: VALUE) -> &mut Self {
        self.controller = Some(controller.into());
        self
    }

    pub fn controller_opt<VALUE: Into<Option<bool>>>(&mut self, controller: VALUE) -> &mut Self {
        self.controller = controller.into();
        self
    }

    pub fn kind<VALUE: Into<String>>(&mut self, kind: VALUE) -> &mut Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn kind_opt<VALUE: Into<Option<String>>>(&mut self, kind: VALUE) -> &mut Self {
        self.kind = kind.into();
        self
    }

    pub fn name<VALUE: Into<String>>(&mut self, name: VALUE) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn name_opt<VALUE: Into<Option<String>>>(&mut self, name: VALUE) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn uid<VALUE: Into<String>>(&mut self, uid: VALUE) -> &mut Self {
        self.uid = Some(uid.into());
        self
    }

    pub fn uid_opt<VALUE: Into<Option<String>>>(&mut self, uid: VALUE) -> &mut Self {
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

/// A builder to build [`Pod`] objects.
///
#[derive(Clone, Default)]
pub struct PodBuilder {
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,
    tolerations: Vec<Toleration>,
    status: Option<PodStatus>,
    containers: Vec<Container>,
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        PodBuilder::default()
    }

    pub fn metadata_default(&mut self) -> &mut Self {
        self.metadata(ObjectMeta::default());
        self
    }

    pub fn metadata_builder<F>(&mut self, f: F) -> OperatorResult<&mut Self>
    where
        F: Fn(&mut ObjectMetaBuilder) -> &mut ObjectMetaBuilder,
    {
        let mut builder = ObjectMetaBuilder::new();
        let builder = f(&mut builder);
        self.metadata = Some(builder.build()?);
        Ok(self)
    }

    pub fn metadata<VALUE: Into<ObjectMeta>>(&mut self, metadata: VALUE) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn metadata_opt<VALUE: Into<Option<ObjectMeta>>>(&mut self, metadata: VALUE) -> &mut Self {
        self.metadata = metadata.into();
        self
    }

    pub fn node_name<VALUE: Into<String>>(&mut self, node_name: VALUE) -> &mut Self {
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
        status.conditions.push(condition);
        self
    }

    pub fn add_container(&mut self, container: Container) -> &mut Self {
        self.containers.push(container);
        self
    }

    /// This will automatically add all required tolerations to target a Stackable agent.
    pub fn add_stackable_agent_tolerations(&mut self) -> &mut Self {
        self.tolerations
            .extend(crate::krustlet::create_tolerations());
        self
    }

    /// Consumes the Builder and returns a constructed Pod
    pub fn build(&self) -> OperatorResult<Pod> {
        // Retrieve all configmaps from all containers and add the relevant volumes to the Pod
        let configmaps = self
            .containers
            .iter()
            .map(|container| {
                container
                    .volume_mounts
                    .iter()
                    .map(|mount| mount.name.clone())
                    .collect::<Vec<String>>()
            })
            .flatten()
            .collect::<HashSet<String>>();

        let volumes = configmaps
            .iter()
            .map(|configmap| Volume {
                name: configmap.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(configmap.clone()),
                    ..ConfigMapVolumeSource::default()
                }),
                ..Volume::default()
            })
            .collect();

        Ok(Pod {
            metadata: match self.metadata {
                None => return Err(Error::MissingObjectKey { key: "metadata" }),
                Some(ref metadata) => metadata.clone(),
            },
            spec: Some(PodSpec {
                containers: self.containers.clone(),
                tolerations: self.tolerations.clone(),
                volumes,
                node_name: self.node_name.clone(),
                ..PodSpec::default()
            }),
            status: self.status.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{
        ConfigMapBuilder, ContainerBuilder, ContainerPortBuilder, EventBuilder, EventType,
        NodeBuilder, ObjectMetaBuilder, PodBuilder,
    };
    use k8s_openapi::api::core::v1::{EnvVar, Pod, VolumeMount};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use std::collections::BTreeMap;

    #[test]
    fn test_configmap_builder() {
        let mut data = BTreeMap::new();
        data.insert("foo".to_string(), "bar".to_string());
        let configmap = ConfigMapBuilder::new()
            .data(data)
            .add_data("bar", "foo")
            .metadata_default()
            .build()
            .unwrap();

        assert!(matches!(configmap.data.get("foo"), Some(bar) if bar == "bar"));
        assert!(matches!(configmap.data.get("bar"), Some(bar) if bar == "foo"));
    }

    #[test]
    fn test_container_builder() {
        let container_port = 10000;
        let container_port_name = "foo_port_name";

        let container = ContainerBuilder::new("testcontainer")
            .add_env_var("foo", "bar")
            .add_configmapvolume("configmap", "/mount")
            .add_container_port(
                ContainerPortBuilder::new(container_port)
                    .name(container_port_name)
                    .build(),
            )
            .add_container_ports(vec![
                ContainerPortBuilder::new(container_port)
                    .name(container_port_name)
                    .build(),
                ContainerPortBuilder::new(container_port)
                    .name(container_port_name)
                    .build(),
            ])
            .build();

        assert_eq!(container.name, "testcontainer");
        assert!(
            matches!(container.env.get(0), Some(EnvVar {name, value: Some(value), ..}) if name == "foo" && value == "bar")
        );
        assert_eq!(container.volume_mounts.len(), 1);
        assert!(
            matches!(container.volume_mounts.get(0), Some(VolumeMount {mount_path, name, ..}) if mount_path == "/mount" && name == "configmap")
        );
        assert!(
            container.ports[0].container_port == i32::from(container_port)
                && container.ports[0].name == Some(container_port_name.to_string())
        );

        assert_eq!(container.ports.len(), 3)
    }

    #[test]
    fn test_container_port_builder() {
        let port: u16 = 10000;
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

        assert_eq!(container_port.container_port, i32::from(port));
        assert_eq!(container_port.name, Some(name.to_lowercase()));
        assert_eq!(container_port.protocol, Some(protocol.to_string()));
        assert_eq!(container_port.host_ip, Some(host_ip.to_string()));
        assert_eq!(container_port.host_port, Some(i32::from(host_port)));
    }

    #[test]
    fn test_event_builder() {
        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("testpod"))
            .unwrap()
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
            .build()
            .unwrap();

        assert_eq!(meta.generate_name, Some("generate_foo".to_string()));
        assert_eq!(meta.name, Some("foo".to_string()));
        assert_eq!(meta.owner_references.len(), 1);
        assert!(
            matches!(meta.owner_references.get(0), Some(OwnerReference { uid, ..}) if uid == "uid")
        );
        assert_eq!(meta.annotations.len(), 1);
        assert_eq!(
            meta.annotations.get(&"foo".to_string()),
            Some(&"bar".to_string())
        );
    }

    #[test]
    fn test_pod_builder() {
        let container = ContainerBuilder::new("containername")
            .image("stackable/zookeeper:2.4.14")
            .command(vec!["zk-server-start.sh".to_string()])
            .args(vec!["{{ configroot }}/conf/zk.properties".to_string()])
            .add_configmapvolume("zk-worker-1", "conf/")
            .build();

        let pod = PodBuilder::new()
            .metadata(ObjectMetaBuilder::new().name("testpod").build().unwrap())
            .add_container(container)
            .node_name("worker-1.stackable.demo")
            .build()
            .unwrap();

        assert_eq!(pod.metadata.name.unwrap(), "testpod");
        assert_eq!(
            pod.spec.unwrap().node_name.unwrap(),
            "worker-1.stackable.demo"
        );

        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("foo"))
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(pod.metadata.name.unwrap(), "foo");
    }
}
