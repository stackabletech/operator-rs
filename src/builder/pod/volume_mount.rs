use k8s_openapi::api::core::v1::VolumeMount;

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
    use super::*;

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
