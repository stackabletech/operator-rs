use std::collections::HashMap;

use k8s_openapi::api::core::v1::ConfigMap;
use kube::Resource;
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
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Security {
    properties: HashMap<String, Option<String>>,
}

pub fn security_config_map<T: Resource>(app: &T, sec: &Security) -> OperatorResult<ConfigMap> {
    ConfigMapBuilder::new()
        .metadata(ObjectMetaBuilder::new().name_and_namespace(app).build())
        .add_data(
            SECURITY_FILE_NAME,
            to_java_properties_string(sec.properties.iter())
                .map_err(|_| Error::JavaProperties(SECURITY_FILE_NAME.to_string()))?,
        )
        .build()
}

pub fn security_system_property(cm_name: &str, mountpoint: &str) -> String {
    format!("-D{SECURITY_SYSTEM_PROPERTY_NAME}={mountpoint}/{cm_name}")
}
