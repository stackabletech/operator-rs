use std::fmt;

use indexmap::IndexMap;
use k8s_openapi::api::core::v1::{
    ConfigMapKeySelector, Container, ContainerPort, EnvVar, EnvVarSource, Lifecycle,
    LifecycleHandler, ObjectFieldSelector, Probe, ResourceRequirements, SecretKeySelector,
    SecurityContext, VolumeMount,
};
use snafu::{ResultExt as _, Snafu};
#[cfg(doc)]
use {k8s_openapi::api::core::v1::PodSpec, std::collections::BTreeMap};

use crate::{
    commons::product_image_selection::ResolvedProductImage,
    validation::{self, is_rfc_1123_label},
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("container name {container_name:?} is invalid"))]
    InvalidContainerName {
        source: validation::Errors,
        container_name: String,
    },

    #[snafu(display(
        "Colliding mountPath {colliding_mount_path:?} in volumeMounts with different content. \
            Existing volume name {existing_volume_name:?}, new volume name {new_volume_name:?}"
    ))]
    MountPathCollision {
        colliding_mount_path: String,
        existing_volume_name: String,
        new_volume_name: String,
    },
}

/// A builder to build [`Container`] objects.
///
/// This will automatically create the necessary volumes and mounts for each `ConfigMap` which is added.
///
/// This struct is often times using an [`IndexMap`] to have consistent ordering (so we don't produce reconcile loops).
/// We are also choosing it over a [`BTreeMap`], because it is easier to debug for users, as logically
/// grouped volumeMounts (e.g. all volumeMounts related to S3) are near each other in the list instead of "just" being
/// sorted alphabetically.
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

    /// The key is the volumeMount mountPath.
    volume_mounts: IndexMap<String, VolumeMount>,
    readiness_probe: Option<Probe>,
    liveness_probe: Option<Probe>,
    startup_probe: Option<Probe>,
    lifecycle: Option<Lifecycle>,
    security_context: Option<SecurityContext>,
}

impl ContainerBuilder {
    pub fn new(name: &str) -> Result<Self> {
        Self::validate_container_name(name)?;
        Ok(ContainerBuilder {
            name: name.to_string(),
            ..ContainerBuilder::default()
        })
    }

    pub fn image(&mut self, image: impl Into<String>) -> &mut Self {
        self.image = Some(image.into());
        self
    }

    pub fn image_pull_policy(&mut self, image_pull_policy: impl Into<String>) -> &mut Self {
        self.image_pull_policy = Some(image_pull_policy.into());
        self
    }

    /// Adds the following container attributes from a [ResolvedProductImage]:
    /// * image
    /// * image_pull_policy
    pub fn image_from_product_image(&mut self, product_image: &ResolvedProductImage) -> &mut Self {
        self.image = Some(product_image.image.clone());
        self.image_pull_policy = Some(product_image.image_pull_policy.clone());
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

    pub fn add_env_var_from_source(
        &mut self,
        name: impl Into<String>,
        value: EnvVarSource,
    ) -> &mut Self {
        self.env.get_or_insert_with(Vec::new).push(EnvVar {
            name: name.into(),
            value_from: Some(value),
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
        self.add_env_var_from_source(
            name,
            EnvVarSource {
                field_ref: Some(ObjectFieldSelector {
                    field_path: field_path.to_string(),
                    ..ObjectFieldSelector::default()
                }),
                ..EnvVarSource::default()
            },
        );
        self
    }

    /// Reference a value from a Secret
    pub fn add_env_var_from_secret(
        &mut self,
        name: impl Into<String>,
        secret_name: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> &mut Self {
        self.add_env_var_from_source(
            name,
            EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: secret_name.into(),
                    key: secret_key.into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        self
    }

    /// Reference a value from a ConfigMap
    pub fn add_env_var_from_config_map(
        &mut self,
        name: impl Into<String>,
        config_map_name: impl Into<String>,
        config_map_key: impl Into<String>,
    ) -> &mut Self {
        self.add_env_var_from_source(
            name,
            EnvVarSource {
                config_map_key_ref: Some(ConfigMapKeySelector {
                    name: config_map_name.into(),
                    key: config_map_key.into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
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

    /// Adds a new [`VolumeMount`] to the container while ensuring that no colliding [`VolumeMount`]
    /// exists.
    ///
    /// A colliding [`VolumeMount`] would have the same mountPath but a different content than
    /// another [`VolumeMount`]. An appropriate error is returned when such a colliding mount path is
    /// encountered.
    ///
    /// ### Note
    ///
    /// Previously, this function unconditionally added [`VolumeMount`]s, which resulted in invalid
    /// [`PodSpec`]s.
    fn add_volume_mount_impl(&mut self, volume_mount: VolumeMount) -> Result<&mut Self> {
        if let Some(existing_volume_mount) = self.volume_mounts.get(&volume_mount.mount_path) {
            if existing_volume_mount != &volume_mount {
                let colliding_mount_path = &volume_mount.mount_path;
                // We don't want to include the details in the error message, but instead trace them
                tracing::error!(
                    colliding_mount_path,
                    ?existing_volume_mount,
                    "Colliding mountPath in volumeMounts with different content"
                );

                MountPathCollisionSnafu {
                    colliding_mount_path,
                    existing_volume_name: &existing_volume_mount.name,
                    new_volume_name: &volume_mount.name,
                }
                .fail()?;
            }
        } else {
            self.volume_mounts
                .insert(volume_mount.mount_path.clone(), volume_mount);
        }

        Ok(self)
    }

    /// Adds a new [`VolumeMount`] to the container while ensuring that no colliding [`VolumeMount`]
    /// exists.
    ///
    /// A colliding [`VolumeMount`] would have the same mountPath but a different content than
    /// another [`VolumeMount`]. An appropriate error is returned when such a colliding mount path is
    /// encountered.
    ///
    /// ### Note
    ///
    /// Previously, this function unconditionally added [`VolumeMount`]s, which resulted in invalid
    /// [`PodSpec`]s.
    pub fn add_volume_mount(
        &mut self,
        name: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<&mut Self> {
        self.add_volume_mount_impl(VolumeMount {
            name: name.into(),
            mount_path: path.into(),
            ..VolumeMount::default()
        })
    }

    /// Adds new [`VolumeMount`]s to the container while ensuring that no colliding [`VolumeMount`]
    /// exists.
    ///
    /// See [`Self::add_volume_mount`] for details.
    pub fn add_volume_mounts(
        &mut self,
        volume_mounts: impl IntoIterator<Item = VolumeMount>,
    ) -> Result<&mut Self> {
        for volume_mount in volume_mounts {
            self.add_volume_mount_impl(volume_mount)?;
        }

        Ok(self)
    }

    pub fn readiness_probe(&mut self, probe: Probe) -> &mut Self {
        self.readiness_probe = Some(probe);
        self
    }

    pub fn liveness_probe(&mut self, probe: Probe) -> &mut Self {
        self.liveness_probe = Some(probe);
        self
    }

    pub fn startup_probe(&mut self, probe: Probe) -> &mut Self {
        self.startup_probe = Some(probe);
        self
    }

    pub fn lifecycle(&mut self, lifecycle: Lifecycle) -> &mut Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    pub fn lifecycle_post_start(&mut self, post_start: LifecycleHandler) -> &mut Self {
        self.lifecycle
            .get_or_insert(Lifecycle::default())
            .post_start = Some(post_start);
        self
    }

    pub fn lifecycle_pre_stop(&mut self, pre_stop: LifecycleHandler) -> &mut Self {
        self.lifecycle.get_or_insert(Lifecycle::default()).pre_stop = Some(pre_stop);
        self
    }

    pub fn security_context(&mut self, context: SecurityContext) -> &mut Self {
        self.security_context = Some(context);
        self
    }

    pub fn resources(&mut self, resources: ResourceRequirements) -> &mut Self {
        self.resources = Some(resources);
        self
    }

    pub fn build(&self) -> Container {
        let volume_mounts = if self.volume_mounts.is_empty() {
            None
        } else {
            Some(self.volume_mounts.values().cloned().collect())
        };

        Container {
            args: self.args.clone(),
            command: self.command.clone(),
            env: self.env.clone(),
            image: self.image.clone(),
            image_pull_policy: self.image_pull_policy.clone(),
            resources: self.resources.clone(),
            name: self.name.clone(),
            ports: self.container_ports.clone(),
            volume_mounts,
            readiness_probe: self.readiness_probe.clone(),
            liveness_probe: self.liveness_probe.clone(),
            startup_probe: self.startup_probe.clone(),
            lifecycle: self.lifecycle.clone(),
            security_context: self.security_context.clone(),
            ..Container::default()
        }
    }

    /// Validates a container name is according to the [RFC 1123](https://www.ietf.org/rfc/rfc1123.txt) standard.
    /// Returns [Ok] if the name is according to the standard, and [Err] if not.
    fn validate_container_name(container_name: &str) -> Result<()> {
        is_rfc_1123_label(container_name).context(InvalidContainerNameSnafu { container_name })
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
            FieldPathEnvVar::Labels(name) => write!(f, "metadata.labels['{name}']"),
            FieldPathEnvVar::Annotations(name) => write!(f, "metadata.annotations['{name}']"),
        }
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::ExecAction;

    use super::*;
    use crate::{
        builder::pod::{
            container::{ContainerBuilder, ContainerPortBuilder, FieldPathEnvVar},
            resources::ResourceRequirementsBuilder,
        },
        commons::resources::ResourceRequirementsType,
        validation::RFC_1123_LABEL_FMT,
    };

    #[test]
    fn builder() {
        let container_port: i32 = 10000;
        let container_port_name = "foo_port_name";
        let container_port_1: i32 = 20000;
        let container_port_name_1 = "bar_port_name";

        let resources = ResourceRequirementsBuilder::new()
            .with_cpu_request("2000m")
            .with_cpu_limit("3000m")
            .with_memory_request("4Gi")
            .with_memory_limit("6Gi")
            .with_resource(ResourceRequirementsType::Limits, "nvidia.com/gpu", "1")
            .build();

        let container = ContainerBuilder::new("testcontainer")
            .expect("ContainerBuilder not created")
            .add_env_var("foo", "bar")
            .add_env_var_from_config_map("envFromConfigMap", "my-configmap", "my-key")
            .add_env_var_from_secret("envFromSecret", "my-secret", "my-key")
            .add_volume_mount("configmap", "/mount")
            .expect("add volume mount")
            .add_container_port(container_port_name, container_port)
            .resources(resources.clone())
            .add_container_ports(vec![
                ContainerPortBuilder::new(container_port_1)
                    .name(container_port_name_1)
                    .build(),
            ])
            .build();

        assert_eq!(container.name, "testcontainer");
        assert!(
            matches!(container.env.as_ref().unwrap().first(), Some(EnvVar {name, value: Some(value), ..}) if name == "foo" && value == "bar")
        );
        assert!(
            matches!(container.env.as_ref().unwrap().get(1), Some(EnvVar {name, value_from: Some(EnvVarSource {config_map_key_ref: Some(ConfigMapKeySelector {name: config_map_name, key: config_map_key, ..}), ..}), ..}) if name == "envFromConfigMap" && config_map_name == "my-configmap" && config_map_key == "my-key")
        );
        assert!(
            matches!(container.env.as_ref().unwrap().get(2), Some(EnvVar {name, value_from: Some(EnvVarSource {secret_key_ref: Some(SecretKeySelector {name: secret_name, key: secret_key, ..}), ..}), ..}) if name == "envFromSecret" && secret_name == "my-secret" && secret_key == "my-key")
        );
        assert_eq!(container.volume_mounts.as_ref().unwrap().len(), 1);
        assert!(
            matches!(container.volume_mounts.as_ref().unwrap().first(), Some(VolumeMount {mount_path, name, ..}) if mount_path == "/mount" && name == "configmap")
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
    fn builder_lifecycle() {
        let post_start = LifecycleHandler {
            exec: Some(ExecAction {
                command: Some(vec!["hello".to_string(), "world".to_string()]),
            }),
            ..Default::default()
        };
        let pre_stop = LifecycleHandler {
            exec: Some(ExecAction {
                command: Some(vec!["bye".to_string(), "bye".to_string()]),
            }),
            ..Default::default()
        };
        let container = ContainerBuilder::new("testcontainer")
            .expect("ContainerBuilder not created")
            .lifecycle_post_start(post_start.clone())
            .lifecycle_pre_stop(pre_stop.clone())
            .build();
        assert_eq!(
            container.lifecycle,
            Some(Lifecycle {
                post_start: Some(post_start),
                pre_stop: Some(pre_stop),
                // Field was added in k8s 1.33 *and* requires the ContainerStopSignals feature gate,
                // so we can't use it yet.
                // See https://kubernetes.io/blog/2025/05/14/kubernetes-v1-33-updates-to-container-lifecycle/
                stop_signal: None,
            })
        );
    }

    #[test]
    fn port_builder() {
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
    pub fn field_ref_env_var_serialization() {
        assert_eq!(
            "metadata.labels['some-label-name']",
            FieldPathEnvVar::Labels("some-label-name".to_string()).to_string()
        );
    }

    // TODO (@Techassi): Use rstest for name validation below
    #[test]
    fn name_max_len() {
        let long_container_name =
            "lengthexceededlengthexceededlengthexceededlengthexceededlengthex";
        assert_eq!(long_container_name.len(), 64); // 63 characters is the limit for container names
        let result = ContainerBuilder::new(long_container_name);
        if let Error::InvalidContainerName {
            container_name,
            source,
        } = result
            .err()
            .expect("Container name exceeding 63 characters should cause an error")
        {
            assert_eq!(container_name, long_container_name);
            assert_eq!(
                source.to_string(),
                "input is 64 bytes long but must be no more than 63"
            )
        }
        // One characters shorter name is valid
        let max_len_container_name: String = long_container_name.chars().skip(1).collect();
        assert_eq!(max_len_container_name.len(), 63);
        assert!(ContainerBuilder::new(&max_len_container_name).is_ok())
    }

    #[test]
    fn name_alphabet_only() {
        ContainerBuilder::new("okname").unwrap();
    }

    #[test]
    fn name_hyphen() {
        assert!(ContainerBuilder::new("name-with-hyphen").is_ok());
        assert_container_builder_err(
            ContainerBuilder::new("ends-with-hyphen-"),
            &format!(r#"regex used for validation is "{RFC_1123_LABEL_FMT}""#),
        );
        assert_container_builder_err(
            ContainerBuilder::new("-starts-with-hyphen"),
            &format!(r#"regex used for validation is "{RFC_1123_LABEL_FMT}""#),
        );
    }

    #[test]
    fn name_contains_number() {
        assert!(ContainerBuilder::new("1name-0-name1").is_ok());
    }

    #[test]
    fn name_contains_underscore() {
        assert!(ContainerBuilder::new("name_name").is_err());
        assert_container_builder_err(
            ContainerBuilder::new("name_name"),
            &format!(
                r#"(e.g. "example-label", or "1-label-1", regex used for validation is "{RFC_1123_LABEL_FMT}""#
            ),
        );
    }

    #[test]
    fn cpu_and_memory_resource_requirements() {
        let resources = ResourceRequirementsBuilder::new()
            .with_cpu_request("2000m")
            .with_cpu_limit("3000m")
            .with_memory_request("4Gi")
            .with_memory_limit("6Gi")
            .with_resource(ResourceRequirementsType::Limits, "nvidia.com/gpu", "1")
            .build();

        let container = ContainerBuilder::new("testcontainer")
            .expect("ContainerBuilder not created")
            .resources(resources.clone())
            .build();

        assert_eq!(container.resources, Some(resources))
    }

    /// Panics if given container builder constructor result is not [Err] with error message
    /// containing expected violation.
    fn assert_container_builder_err(
        result: Result<ContainerBuilder, Error>,
        expected_err_contains: &str,
    ) {
        if let Error::InvalidContainerName {
            container_name: _,
            source,
        } = result
            .err()
            .expect("Container name exceeding 63 characters should cause an error")
        {
            assert!(dbg!(source.to_string()).contains(dbg!(expected_err_contains)));
        }
    }
}
