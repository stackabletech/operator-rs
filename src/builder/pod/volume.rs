use k8s_openapi::{
    api::core::v1::{
        CSIVolumeSource, ConfigMapVolumeSource, DownwardAPIVolumeSource, EmptyDirVolumeSource,
        HostPathVolumeSource, PersistentVolumeClaimVolumeSource, ProjectedVolumeSource,
        SecretVolumeSource, Volume,
    },
    apimachinery::pkg::api::resource::Quantity,
};

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

#[cfg(test)]
mod tests {
    use crate::builder::pod::volume::VolumeBuilder;
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
}
