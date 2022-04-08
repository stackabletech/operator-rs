use crate::builder::SecretOperatorVolumeSourceBuilder;
use k8s_openapi::api::core::v1::CSIVolumeSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretClassVolume {
    /// [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html) containing the LDAP bind credentials
    pub secret_class: String,
    /// [Scope](https://docs.stackable.tech/secret-operator/scope.html) of the [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html)
    pub scope: Option<SecretClassVolumeScope>,
}

impl SecretClassVolume {
    pub fn to_csi_volume(&self) -> CSIVolumeSource {
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

        secret_operator_volume_builder.build()
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
    fn test_secret_class_volume_to_csi_volume() {
        let secret_class_volume = SecretClassVolume {
            secret_class: "myclass".to_string(), // pragma: allowlist secret
            scope: Some(SecretClassVolumeScope {
                pod: true,
                node: false,
                services: vec!["myservice".to_string()],
            }),
        }
        .to_csi_volume();

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
            secret_class_volume.volume_attributes.unwrap()
        );
    }
}
