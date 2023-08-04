use std::collections::HashMap;

use k8s_openapi::api::core::v1::ConfigMap;
use kube::{Resource, ResourceExt};
use product_config::writer::to_java_properties_string;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::{ConfigMapBuilder, ObjectMetaBuilder},
    error::Error,
    error::OperatorResult,
};

/// JVM configuration management.

pub const SECURITY_SYSTEM_PROPERTY_NAME: &str = "java.security.properties";
pub const SECURITY_FILE_NAME: &str = "security.properties";

// This is a preliminary interface. Operators should be ignorant to the actual structure.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Security {
    properties: HashMap<String, Option<String>>,
}

// TODO: decide on the defaults here
impl Default for Security {
    fn default() -> Self {
        Self {
            properties: vec![
                (
                    "networkaddress.cache.ttl".to_string(),
                    Some("10".to_string()),
                ),
                (
                    "networkaddress.cache.negative.ttl".to_string(),
                    Some("10".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}

pub fn security_config_map<T: Resource>(app: &T, sec: &Security) -> OperatorResult<ConfigMap> {
    ConfigMapBuilder::new()
        .metadata(
            ObjectMetaBuilder::new()
                .name_and_namespace(app)
                .name(format!("{}-jvm-security", app.name_any()))
                .build(),
        )
        .add_data(
            SECURITY_FILE_NAME,
            to_java_properties_string(sec.properties.iter())
                .map_err(|_| Error::JavaProperties(SECURITY_FILE_NAME.to_string()))?,
        )
        .build()
}

pub fn default_security_config_map<T: Resource>(app: &T) -> OperatorResult<ConfigMap> {
    security_config_map(app, &Security::default())
}

pub fn security_system_property(cm_name: &str, mountpoint: &str) -> String {
    format!("-D{SECURITY_SYSTEM_PROPERTY_NAME}={mountpoint}/{cm_name}")
}
