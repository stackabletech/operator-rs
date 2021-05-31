use crate::error::OperatorResult;
use crate::labels;
use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, Container, EnvVar, Node, Pod, PodSpec, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference, Time};
use kube::Resource;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Default)]
pub struct OwnerreferenceBuilder {
    api_version: Option<String>,
    block_owner_deletion: Option<bool>,
    controller: Option<bool>,
    kind: Option<String>,
    name: Option<String>,
    uid: Option<String>,
}

impl OwnerreferenceBuilder {
    pub fn new() -> OwnerreferenceBuilder {
        OwnerreferenceBuilder::default()
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
                None => return Err(crate::error::Error::MissingObjectKey { key: "api_version" }),
                Some(ref api_version) => api_version.clone(),
            },
            block_owner_deletion: self.block_owner_deletion.clone(),
            controller: self.controller.clone(),
            kind: match self.kind {
                None => return Err(crate::error::Error::MissingObjectKey { key: "kind" }),
                Some(ref kind) => kind.clone(),
            },
            name: match self.name {
                None => return Err(crate::error::Error::MissingObjectKey { key: "name" }),
                Some(ref name) => name.clone(),
            },
            uid: match self.uid {
                None => return Err(crate::error::Error::MissingObjectKey { key: "uid" }),
                Some(ref uid) => uid.clone(),
            },
        })
    }
}

#[derive(Clone, Default)]
pub struct ObjectmetaBuilder {
    name: Option<String>,
    namespace: Option<String>,
    ownerreference: Option<OwnerReference>,
    labels: BTreeMap<String, String>,
}

impl ObjectmetaBuilder {
    pub fn new() -> ObjectmetaBuilder {
        ObjectmetaBuilder::default()
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
                // TODO: map
                Some(ref ownerreference) => Some(vec![ownerreference.clone()]),
                None => None,
            },
            labels: Some(self.labels.clone()),

            ..ObjectMeta::default()
        })
    }
}

#[derive(Clone, Default)]
pub struct PodBuilder {
    metadata: Option<ObjectMeta>,
    node_name: Option<String>,

    #[cfg(test)]
    deletion_timestamp: Option<Time>,

    containers: Vec<Container>,
    configmaps: HashSet<String>,
}

impl PodBuilder {
    pub fn new() -> PodBuilder {
        PodBuilder::default()
    }

    pub fn objectmeta<VALUE: Into<ObjectMeta>>(&mut self, metadata: VALUE) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn objectmeta_opt<VALUE: Into<Option<ObjectMeta>>>(
        &mut self,
        metadata: VALUE,
    ) -> &mut Self {
        self.metadata = metadata.into();
        self
    }

    pub fn new_objectmeta() -> ObjectmetaBuilder {
        ObjectmetaBuilder::new()
    }

    pub fn node_name<VALUE: Into<String>>(&mut self, node_name: VALUE) -> &mut Self {
        self.node_name = Some(node_name.into());
        self
    }

    /*
    pub fn phase(&mut self, phase: &str) -> &mut Self {
        let mut status = self.pod.status.get_or_insert_with(PodStatus::default);
        status.phase = Some(phase.to_string());
        self
    }


    pub fn with_condition(&mut self, condition_type: &str, condition_status: &str) -> &mut Self {
        let status = self.pod.status.get_or_insert_with(PodStatus::default);
        let conditions = status.conditions.get_or_insert_with(Vec::new);
        let condition = PodCondition {
            status: condition_status.to_string(),
            type_: condition_type.to_string(),
            ..PodCondition::default()
        };
        conditions.push(condition);
        self
    }


    #[cfg(test)]
    pub fn deletion_timestamp<VALUE: Into<Time>>(
        &mut self,
        deletion_timestamp: VALUE,
    ) -> &mut Self {
        self.deletion_timestamp = Some(deletion_timestamp.into());
        self
    }
     */

    pub fn add_container(&mut self, container: Container) -> &mut Self {
        self.containers.push(container);
        self
    }

    /// Consumes the Builder and returns a constructed Pod
    pub fn build(&self) -> Pod {
        // Retrieve all configmaps from all containers and add the relevant volumes to the Pod
        /*
        let mount_names = self
            .containers
            .iter()
            .map(|container| match &container.volume_mounts {
                None => vec![],
                Some(mounts) => mounts
                    .iter()
                    .map(|mount| (mount.name.clone(), mount.mount_path.clone()))
                    .collect(),
            })
            .collect::<HashMap<String, String>>();


         */
        Pod {
            spec: Some(PodSpec {
                // TODO: See https://github.com/colin-kiegel/rust-derive-builder for now we could use an unwrap, this is just an example
                node_name: match self.node_name {
                    Some(ref node_name) => Some(node_name.clone()),
                    None => {
                        panic!("Uninitialized field");
                    }
                },

                ..PodSpec::default()
            }),
            ..Pod::default()
        }
    }
}

#[derive(Default)]
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

    pub fn build(self) -> Container {
        let mut volumes = vec![];
        let mut volume_mounts = vec![];
        for (mount_path, configmap_name) in self.configmaps {
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
                name: configmap_name,
                mount_path,
                ..VolumeMount::default()
            };
            volume_mounts.push(volume_mount);
        }

        Container {
            image: self.image,
            name: self.name,
            env: Some(self.env),
            command: Some(self.command),
            args: Some(self.args),
            volume_mounts: Some(volume_mounts),
            ..Container::default()
        }
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

#[cfg(test)]
mod tests {
    use crate::builder::{ContainerBuilder, OwnerreferenceBuilder, PodBuilder};

    #[test]
    fn test() {
        let owner_reference = OwnerreferenceBuilder::new().name(Some("foo"));

        let mut container = ContainerBuilder::new("containername")
            .image("stackable/zookeeper:2.4.14")
            .command(vec!["zk-server-start.sh".to_string()])
            .args(vec!["{{ configroot }}/conf/zk.properties".to_string()])
            .add_config_map("zk-worker-1", "conf/")
            .build();

        let pod = PodBuilder::new()
            .name("testpod")
            .add_container(container)
            .node_name("worker-1.stackable.demo")
            .build();

        assert_eq!(pod.metadata.name, "testpod");
        assert_eq!(pod.spec.node_name, "worker-1.stackable.demo");
    }
}
