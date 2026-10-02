use const_format::concatcp;
use k8s_openapi::api::core::v1::{SecretVolumeSource, Volume, VolumeMount};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::pod::{
        PodBuilder,
        container::ContainerBuilder,
        volume::{
            SecretFormat, SecretOperatorVolumeSourceBuilder, VolumeBuilder, VolumeMountBuilder,
        },
    },
    commons::secret_class::SecretClassVolumeProvisionParts,
    constants::secret::SECRET_BASE_PATH,
    v2::types::kubernetes::{SecretClassName, SecretName},
};

/// Name of the volume holding the TLS client certificate.
pub const VOLUME_NAME: &str = "tls-client-cert";

/// Mount path for the TLS client certificate.
pub const MOUNT_PATH: &str = concatcp!(SECRET_BASE_PATH, "/", VOLUME_NAME);

/// Source of a TLS client certificate: a secret-operator SecretClass or a static Secret.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsClientCredential {
    /// An AutoTLS SecretClass used to provision the certificate.
    SecretClass(SecretClassName),

    /// A static Secret holding the certificate in the keys `tls.crt` and `tls.key` (PEM), e.g. as a
    /// Secret of type `kubernetes.io/tls`.
    Secret(SecretName),
}

impl TlsClientCredential {
    /// Adds the certificate volume to the Pod and mounts it into all given containers.
    /// - TlsClientCredential::Secret mounts the Secret
    /// - TlsClientCredential::SecretClass adds a secret-operator volume
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) {
        let (volumes, mounts) = self.volumes_and_mounts();
        pod_builder
            .add_volumes(volumes)
            .expect("Only a single platform access authentication variant can be chosen.");
        for container_builder in container_builders {
            container_builder
                .add_volume_mounts(mounts.clone())
                .expect("Only a single platform access authentication variant can be chosen.");
        }
    }

    fn volumes_and_mounts(&self) -> (Vec<Volume>, Vec<VolumeMount>) {
        let volume = match self {
            Self::SecretClass(secret_class) => VolumeBuilder::new(VOLUME_NAME)
                .ephemeral(
                    SecretOperatorVolumeSourceBuilder::new(
                        secret_class,
                        SecretClassVolumeProvisionParts::PublicPrivate,
                    )
                    .with_pod_scope()
                    .with_format(SecretFormat::TlsPem)
                    .build()
                    .expect("the annotations are built from a valid SecretClass name"),
                )
                .build(),
            Self::Secret(secret) => VolumeBuilder::new(VOLUME_NAME)
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

    #[test]
    fn secret_class_credential_is_provisioned_by_secret_operator() {
        let credential = TlsClientCredential::SecretClass(SecretClassName::from_str_unsafe("tls"));

        let (volumes, mounts) = credential.volumes_and_mounts();
        let volumes = serde_json::to_value(volumes).expect("serializable");

        assert_eq!(volumes[0]["name"], "tls-client-cert");
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
            serde_json::to_value(mounts).expect("serializable"),
            json!([{"mountPath": "/stackable/secrets/tls-client-cert", "name": "tls-client-cert"}])
        );
    }

    #[test]
    fn static_secret_credential_is_mounted_directly() {
        let credential = TlsClientCredential::Secret(SecretName::from_str_unsafe("my-cert"));

        let (volumes, mounts) = credential.volumes_and_mounts();

        assert_eq!(
            serde_json::to_value(volumes).expect("serializable"),
            json!([{"name": "tls-client-cert", "secret": {"secretName": "my-cert"}}])
        );
        assert_eq!(
            serde_json::to_value(mounts).expect("serializable"),
            json!([{"mountPath": "/stackable/secrets/tls-client-cert", "name": "tls-client-cert"}])
        );
    }
}
