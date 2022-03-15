use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, Probe,
    ResourceRequirements, SecurityContext, VolumeMount,
};
use std::fmt;

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
    resources: Option<ResourceRequirements>,
    volume_mounts: Option<Vec<VolumeMount>>,
    readiness_probe: Option<Probe>,
    liveness_probe: Option<Probe>,
    security_context: Option<SecurityContext>,
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

    /// Used for pushing down attributes like the Pod's namespace into the containers.
    pub fn add_env_var_from_field_path(
        &mut self,
        name: impl Into<String>,
        field_path: FieldPathEnvVar,
    ) -> &mut Self {
        self.env.get_or_insert_with(Vec::new).push(EnvVar {
            name: name.into(),
            value_from: Some(EnvVarSource {
                field_ref: Some(ObjectFieldSelector {
                    field_path: field_path.to_string(),
                    ..ObjectFieldSelector::default()
                }),
                ..EnvVarSource::default()
            }),
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

    pub fn resources(&mut self, resources: ResourceRequirements) -> &mut Self {
        self.resources = Some(resources);
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

    pub fn security_context(&mut self, context: SecurityContext) -> &mut Self {
        self.security_context = Some(context);
        self
    }

    pub fn build(&self) -> Container {
        Container {
            args: self.args.clone(),
            command: self.command.clone(),
            env: self.env.clone(),
            image: self.image.clone(),
            image_pull_policy: self.image_pull_policy.clone(),
            resources: self.resources.clone(),
            name: self.name.clone(),
            ports: self.container_ports.clone(),
            volume_mounts: self.volume_mounts.clone(),
            readiness_probe: self.readiness_probe.clone(),
            liveness_probe: self.liveness_probe.clone(),
            security_context: self.security_context.clone(),
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
            name: self.name.clone().map(|s| s.to_lowercase()),
            host_ip: self.host_ip.clone(),
            protocol: self.protocol.clone(),
            host_port: self.host_port,
        }
    }
}

/// Downward API capabilities available via `fieldRef`
/// See: <https://kubernetes.io/docs/tasks/inject-data-application/downward-api-volume-expose-pod-information/#capabilities-of-the-downward-api>
#[derive(Debug)]
pub enum FieldPathEnvVar {
    Name,
    Namespace,
    UID,
    Labels(String),
    Annotations(String),
}

impl fmt::Display for FieldPathEnvVar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FieldPathEnvVar::Name => write!(f, "metadata.name"),
            FieldPathEnvVar::Namespace => write!(f, "metadata.namespace"),
            FieldPathEnvVar::UID => write!(f, "metadata.uid"),
            FieldPathEnvVar::Labels(name) => write!(f, "metadata.labels['{}']", name),
            FieldPathEnvVar::Annotations(name) => write!(f, "metadata.annotations['{}']", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::pod::container::{ContainerBuilder, ContainerPortBuilder, FieldPathEnvVar};
    use k8s_openapi::{
        api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity,
    };

    #[test]
    fn test_container_builder() {
        let container_port: i32 = 10000;
        let container_port_name = "foo_port_name";
        let container_port_1: i32 = 20000;
        let container_port_name_1 = "bar_port_name";
        let resources = ResourceRequirements {
            limits: Some(
                [
                    ("cpu".to_string(), Quantity("3000m".to_string())),
                    ("memory".to_string(), Quantity("6Gi".to_string())),
                    ("nvidia.com/gpu".to_string(), Quantity("1".to_string())),
                ]
                .into(),
            ),
            requests: Some(
                [
                    ("cpu".to_string(), Quantity("2000m".to_string())),
                    ("memory".to_string(), Quantity("4Gi".to_string())),
                ]
                .into(),
            ),
        };

        let container = ContainerBuilder::new("testcontainer")
            .add_env_var("foo", "bar")
            .add_volume_mount("configmap", "/mount")
            .add_container_port(container_port_name, container_port)
            .resources(resources.clone())
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
        assert_eq!(container.resources, Some(resources));
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
    pub fn test_field_ref_env_var_serialization() {
        assert_eq!(
            "metadata.labels['some-label-name']",
            FieldPathEnvVar::Labels("some-label-name".to_string()).to_string()
        );
    }
}
