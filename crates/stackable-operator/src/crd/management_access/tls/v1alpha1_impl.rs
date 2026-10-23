use k8s_openapi::api::core::v1::{SecretVolumeSource, Volume, VolumeMount};
use snafu::{ResultExt as _, Snafu};

use crate::{
    builder::{
        self,
        pod::{
            PodBuilder,
            container::ContainerBuilder,
            volume::{
                SecretFormat, SecretOperatorVolumeSourceBuilder, VolumeBuilder, VolumeMountBuilder,
            },
        },
    },
    commons::secret_class::SecretClassVolumeProvisionParts,
    crd::management_access::tls::{MOUNT_PATH, VOLUME_NAME, v1alpha1::TlsClientCredential},
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to add the TLS client certificate volume"))]
    AddVolumes { source: builder::pod::Error },

    #[snafu(display("failed to add the TLS client certificate volumeMount"))]
    AddVolumeMounts {
        source: builder::pod::container::Error,
    },
}

impl TlsClientCredential {
    /// Adds the certificate volume to the Pod and mounts it into all given containers.
    /// - TlsClientCredential::SecretName mounts the Secret
    /// - TlsClientCredential::SecretClassName adds a secret-operator volume
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) -> Result<(), Error> {
        let (volumes, mounts) = self.volumes_and_mounts();
        pod_builder.add_volumes(volumes).context(AddVolumesSnafu)?;
        for container_builder in container_builders {
            container_builder
                .add_volume_mounts(mounts.clone())
                .context(AddVolumeMountsSnafu)?;
        }

        Ok(())
    }

    fn volumes_and_mounts(&self) -> (Vec<Volume>, Vec<VolumeMount>) {
        let volume = match self {
            Self::SecretClassName(secret_class) => VolumeBuilder::new(VOLUME_NAME)
                .ephemeral(
                    SecretOperatorVolumeSourceBuilder::new(
                        secret_class,
                        SecretClassVolumeProvisionParts::PublicPrivate,
                    )
                    .with_pod_scope()
                    .with_format(SecretFormat::TlsPem)
                    .build()
                    .expect("the annotations are built from a valid SecretClass name and constant values"),
                )
                .build(),
            Self::SecretName(secret) => VolumeBuilder::new(VOLUME_NAME)
                .secret(SecretVolumeSource {
                    secret_name: Some(secret.to_string()),
                    ..SecretVolumeSource::default()
                })
                .build(),
        };
        let mount = VolumeMountBuilder::new(VOLUME_NAME, MOUNT_PATH).build();
        (vec![volume], vec![mount])
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::v2::types::kubernetes::{SecretClassName, SecretName};

    #[test]
    fn secret_class_credential_is_provisioned_by_secret_operator() {
        let credential =
            TlsClientCredential::SecretClassName(SecretClassName::from_str_unsafe("tls"));

        let (volumes, mounts) = credential.volumes_and_mounts();
        let volumes = serde_json::to_value(volumes).expect("the volumes should be serializable");

        assert_eq!(volumes[0]["name"], "tls-management-access");
        assert_eq!(
            volumes[0]["ephemeral"]["volumeClaimTemplate"]["metadata"]["annotations"],
            json!({
                "secrets.stackable.tech/class": "tls",
                "secrets.stackable.tech/format": "tls-pem",
                "secrets.stackable.tech/provision-parts": "public-private",
                "secrets.stackable.tech/scope": "pod"
            })
        );
        assert_eq!(
            serde_json::to_value(mounts).expect("the volume mounts should be serializable"),
            json!([{"mountPath": "/stackable/secrets/tls-management-access", "name": "tls-management-access"}])
        );
    }

    #[test]
    fn static_secret_credential_is_mounted_directly() {
        let credential = TlsClientCredential::SecretName(SecretName::from_str_unsafe("my-cert"));

        let (volumes, mounts) = credential.volumes_and_mounts();

        assert_eq!(
            serde_json::to_value(volumes).expect("the volumes should be serializable"),
            json!([{"name": "tls-management-access", "secret": {"secretName": "my-cert"}}])
        );
        assert_eq!(
            serde_json::to_value(mounts).expect("the volume mounts should be serializable"),
            json!([{"mountPath": "/stackable/secrets/tls-management-access", "name": "tls-management-access"}])
        );
    }
}
