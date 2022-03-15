use k8s_openapi::api::core::v1::VolumeMount;
use k8s_openapi::{
    api::core::v1::{
        CSIVolumeSource, ConfigMapVolumeSource, DownwardAPIVolumeSource, EmptyDirVolumeSource,
        HostPathVolumeSource, PersistentVolumeClaimVolumeSource, ProjectedVolumeSource,
        SecretVolumeSource, Volume,
    },
    apimachinery::pkg::api::resource::Quantity,
};
use std::collections::BTreeMap;

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
    Csi(CSIVolumeSource),
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

    pub fn csi(&mut self, csi: impl Into<CSIVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Csi(csi.into());
        self
    }

    /// Consumes the Builder and returns a constructed Volume
    pub fn build(&self) -> Volume {
        let name = self.name.clone();
        match &self.volume_source {
            VolumeSource::ConfigMap(cm) => Volume {
                name,
                config_map: Some(cm.clone()),
                ..Volume::default()
            },
            VolumeSource::DownwardApi(downward_api) => Volume {
                name,
                downward_api: Some(downward_api.clone()),
                ..Volume::default()
            },
            VolumeSource::EmptyDir(empty_dir) => Volume {
                name,
                empty_dir: Some(empty_dir.clone()),
                ..Volume::default()
            },
            VolumeSource::HostPath(host_path) => Volume {
                name,
                host_path: Some(host_path.clone()),
                ..Volume::default()
            },
            VolumeSource::PersistentVolumeClaim(pvc) => Volume {
                name,
                persistent_volume_claim: Some(pvc.clone()),
                ..Volume::default()
            },
            VolumeSource::Projected(projected) => Volume {
                name,
                projected: Some(projected.clone()),
                ..Volume::default()
            },
            VolumeSource::Secret(secret) => Volume {
                name,
                secret: Some(secret.clone()),
                ..Volume::default()
            },
            VolumeSource::Csi(csi) => Volume {
                name,
                csi: Some(csi.clone()),
                ..Volume::default()
            },
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

#[derive(Clone)]
pub struct SecretOperatorVolumeSourceBuilder {
    secret_class: String,
    scopes: Vec<SecretOperatorVolumeScope>,
}

impl SecretOperatorVolumeSourceBuilder {
    pub fn new(secret_class: impl Into<String>) -> Self {
        Self {
            secret_class: secret_class.into(),
            scopes: Vec::new(),
        }
    }

    pub fn with_node_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Node);
        self
    }

    pub fn with_pod_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Pod);
        self
    }

    pub fn with_service_scope(&mut self, name: impl Into<String>) -> &mut Self {
        self.scopes
            .push(SecretOperatorVolumeScope::Service { name: name.into() });
        self
    }

    pub fn build(&self) -> CSIVolumeSource {
        let mut attrs = BTreeMap::from([(
            "secrets.stackable.tech/class".to_string(),
            self.secret_class.clone(),
        )]);

        if !self.scopes.is_empty() {
            let mut scopes = String::new();
            for scope in self.scopes.iter() {
                if !scopes.is_empty() {
                    scopes.push(',');
                };
                match scope {
                    SecretOperatorVolumeScope::Node => scopes.push_str("node"),
                    SecretOperatorVolumeScope::Pod => scopes.push_str("pod"),
                    SecretOperatorVolumeScope::Service { name } => {
                        scopes.push_str("service=");
                        scopes.push_str(name);
                    }
                }
            }
            attrs.insert("secrets.stackable.tech/scope".to_string(), scopes);
        }

        CSIVolumeSource {
            driver: "secrets.stackable.tech".to_string(),
            volume_attributes: Some(attrs),
            ..CSIVolumeSource::default()
        }
    }
}

#[derive(Clone)]
enum SecretOperatorVolumeScope {
    Node,
    Pod,
    Service { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

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
}
