use k8s_openapi::api::core::v1::{EphemeralVolumeSource, Volume};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::builder::{
    SecretOperatorVolumeSourceBuilder, SecretOperatorVolumeSourceBuilderError, VolumeBuilder,
};

#[derive(Debug, Snafu)]
pub enum SecretClassVolumeError {
    #[snafu(display("failed to build secret operator volume"))]
    SecretOperatorVolume {
        source: SecretOperatorVolumeSourceBuilderError,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretClassVolume {
    /// [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html) containing the LDAP bind credentials
    pub secret_class: String,

    /// [Scope](https://docs.stackable.tech/secret-operator/scope.html) of the [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html)
    pub scope: Option<SecretClassVolumeScope>,
}

impl SecretClassVolume {
    pub fn new(secret_class: String, scope: Option<SecretClassVolumeScope>) -> Self {
        Self {
            secret_class,
            scope,
        }
    }

    pub fn to_ephemeral_volume_source(
        &self,
    ) -> Result<EphemeralVolumeSource, SecretClassVolumeError> {
        let mut secret_operator_volume_builder =
            SecretOperatorVolumeSourceBuilder::new(&self.secret_class);

        if let Some(scope) = &self.scope {
            if scope.pod {
                secret_operator_volume_builder.with_pod_scope();
            }
            if scope.node {
                secret_operator_volume_builder.with_node_scope();
            }
            for service in &scope.services {
                secret_operator_volume_builder.with_service_scope(service);
            }
        }

        secret_operator_volume_builder
            .build()
            .context(SecretOperatorVolumeSnafu)
    }

    pub fn to_volume(&self, volume_name: &str) -> Result<Volume, SecretClassVolumeError> {
        let ephemeral = self.to_ephemeral_volume_source()?;
        Ok(VolumeBuilder::new(volume_name).ephemeral(ephemeral).build())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretClassVolumeScope {
    #[serde(default)]
    pub pod: bool,
    #[serde(default)]
    pub node: bool,
    #[serde(default)]
    pub services: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_secret_class_volume_to_csi_volume_source() {
        let secret_class_volume_source = SecretClassVolume {
            secret_class: "myclass".to_string(), // pragma: allowlist secret
            scope: Some(SecretClassVolumeScope {
                pod: true,
                node: false,
                services: vec!["myservice".to_string()],
            }),
        }
        .to_ephemeral_volume_source()
        .unwrap();

        let expected_volume_attributes = BTreeMap::from([
            (
                "secrets.stackable.tech/class".to_string(),
                "myclass".to_string(),
            ),
            (
                "secrets.stackable.tech/scope".to_string(),
                "pod,service=myservice".to_string(),
            ),
        ]);

        assert_eq!(
            expected_volume_attributes,
            secret_class_volume_source
                .volume_claim_template
                .unwrap()
                .metadata
                .unwrap()
                .annotations
                .unwrap()
        );
    }
}
