//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in defaults that make sense.
use crate::error::{Error, OperatorResult};
use crate::labels;
use k8s_openapi::api::core::v1::{
    ConfigMap, ConfigMapVolumeSource, Container, EnvVar, Node, Pod, PodCondition, PodSpec,
    PodStatus, Toleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use k8s_openapi::ByteString;
use kube::{Resource, ResourceExt};
use std::collections::{BTreeMap, HashMap, HashSet};

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
            block_owner_deletion: self.block_owner_deletion.clone(),
            controller: self.controller.clone(),
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

/// A builder to build [`ObjectMeta`] objects.
///
/// Of special interest is the [`Self::ownerreference_from_resource()`] function.
/// Note: This builder only supports a single `OwnerReference`.
///
/// It is strongly recommended to always call [`Self::with_recommended_labels()`]!
#[derive(Clone, Default)]
pub struct ObjectMetaBuilder {
    name: Option<String>,
    namespace: Option<String>,
    ownerreference: Option<OwnerReference>,
    labels: BTreeMap<String, String>,
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
    /// It is recommended to always call this method and is mostly not required to make testing easier.
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
            name: self.name.clone(),
            namespace: self.namespace.clone(),
            owner_references: match self.ownerreference {
                Some(ref ownerreference) => vec![ownerreference.clone()],
                None => vec![],
            },
            labels: self.labels.clone(),

            ..ObjectMeta::default()
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
            ..Pod::default()
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
}

impl ContainerBuilder {
    pub fn new(name: &str) -> Self {
        ContainerBuilder {
            name: name.to_string(),
            ..ContainerBuilder::default()
        }
    }

    pub fn image(&mut self, image: &str) -> &mut Self {
        self.image = Some(image.to_string());
        self
    }

    pub fn add_env_var(&mut self, name: &str, value: &str) -> &mut Self {
        self.env.push(EnvVar {
            name: name.to_string(),
            value: Some(value.to_string()),
            ..EnvVar::default()
        });
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

    pub fn add_config_map(&mut self, configmap_name: &str, mount_path: &str) -> &mut Self {
        self.configmaps
            .insert(mount_path.to_string(), configmap_name.to_string());
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
            ..Container::default()
        }
    }
}

/// A builder to build [`ConfigMap`] objects.
#[derive(Clone, Default)]
pub struct ConfigMapBuilder {
    metadata: Option<ObjectMeta>,
    binary_data: BTreeMap<String, ByteString>,
    data: BTreeMap<String, String>,
    immutable: Option<bool>,
}

impl ConfigMapBuilder {
    pub fn new() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
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

/// A builder to build [`Node`] objects.
///
/// This is mainly useful for tests.
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
        self.node.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{
        ConfigMapBuilder, ContainerBuilder, ObjectMetaBuilder, OwnerReferenceBuilder, PodBuilder,
    };

    #[test]
    fn test_configmap_builder() {
        let builder = ConfigMapBuilder::new();
    }

    #[test]
    fn test() {
        let owner_reference = OwnerReferenceBuilder::new().name("foo");

        let mut container = ContainerBuilder::new("containername")
            .image("stackable/zookeeper:2.4.14")
            .command(vec!["zk-server-start.sh".to_string()])
            .args(vec!["{{ configroot }}/conf/zk.properties".to_string()])
            .add_config_map("zk-worker-1", "conf/")
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
