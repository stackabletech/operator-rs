use k8s_openapi::api::core::v1::{EnvVar, VolumeMount};
use snafu::{ResultExt, Snafu};
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::{
    builder::pod::{container::ContainerBuilder, resources::ResourceRequirementsBuilder},
    commons::product_image_selection::ResolvedProductImage,
};

use super::spec::{GIT_SYNC_ROOT, GitSync};

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("invalid container name"))]
    InvalidContainerName {
        source: crate::builder::pod::container::Error,
    },

    #[snafu(display("failed to add needed volumeMount"))]
    AddVolumeMount {
        source: crate::builder::pod::container::Error,
    },
}

pub fn build_gitsync_container(
    resolved_product_image: &ResolvedProductImage,
    gitsync: &&GitSync,
    one_time: bool,
    name: &str,
    env_vars: Vec<EnvVar>,
    volume_name: &str,
    volume_mounts: Vec<VolumeMount>,
) -> Result<k8s_openapi::api::core::v1::Container, Error> {
    let gitsync_container = ContainerBuilder::new(name)
        .context(InvalidContainerNameSnafu)?
        .add_env_vars(env_vars)
        .image_from_product_image(resolved_product_image)
        .command(vec![
            "/bin/bash".to_string(),
            "-x".to_string(),
            "-euo".to_string(),
            "pipefail".to_string(),
            "-c".to_string(),
        ])
        .args(vec![gitsync.get_args(one_time).join("\n")])
        .add_volume_mount(volume_name, GIT_SYNC_ROOT)
        .context(AddVolumeMountSnafu)?
        .add_volume_mounts(volume_mounts)
        .context(AddVolumeMountSnafu)?
        .resources(
            ResourceRequirementsBuilder::new()
                .with_cpu_request("100m")
                .with_cpu_limit("200m")
                .with_memory_request("64Mi")
                .with_memory_limit("64Mi")
                .build(),
        )
        .build();
    Ok(gitsync_container)
}
