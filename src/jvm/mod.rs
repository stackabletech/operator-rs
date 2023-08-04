//! JVM configuration management.
//! Currently it supports only JVM security properties.
use std::collections::{BTreeMap, HashMap};

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

/// Java system property that points to the location of the custom security configuration file
pub const SECURITY_SYSTEM_PROPERTY_NAME: &str = "java.security.properties";
/// Name of the custom security configuration file
pub const SECURITY_FILE_NAME: &str = "security.properties";

/// Seconds to cache positive DNS results.
pub const PROP_NAME_NET_ADDR_CACHE_TTL: &str = "networkaddress.cache.ttl";
/// Seconds to cache negative DNS results.
pub const PROP_NAME_NET_ADDR_CACHE_NEGATIVE_TTL: &str = "networkaddress.cache.negative.ttl";

/// TODO: This is a preliminary interface. Operators should be ignorant to the actual structure.
/// Structure that holds Java security properties.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Security {
    properties: HashMap<String, Option<String>>,
}

/// TODO: decide on the defaults here
impl Default for Security {
    fn default() -> Self {
        Self {
            properties: vec![
                (
                    PROP_NAME_NET_ADDR_CACHE_TTL.to_string(),
                    Some("10".to_string()),
                ),
                (
                    PROP_NAME_NET_ADDR_CACHE_NEGATIVE_TTL.to_string(),
                    Some("10".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}

/// Generate a config map for the given Security object.
///
/// If no security object is given, the default from this module is used.
///
/// The generated config map data contains a single entry with the name and contents
/// of the custom security configuration file.
pub fn security_config_map<T: Resource<DynamicType = ()>>(
    owner: &T,
    labels: BTreeMap<String, String>,
    security_opt: &Option<Security>,
) -> OperatorResult<ConfigMap> {
    let props = match security_opt {
        Some(sec) => sec.properties.clone(),
        _ => Security::default().properties,
    };

    ConfigMapBuilder::new()
        .metadata(
            ObjectMetaBuilder::new()
                .name_and_namespace(owner)
                .name(format!("{}-jvm-security", owner.name_any()))
                .ownerreference_from_resource(owner, None, Some(true))?
                .with_labels(labels)
                .build(),
        )
        .add_data(
            SECURITY_FILE_NAME,
            to_java_properties_string(props.iter())
                .map_err(|_| Error::JavaProperties(SECURITY_FILE_NAME.to_string()))?,
        )
        .build()
}

/// Java CLI argument for custom security configuration.
pub fn security_system_property(mountpoint: &str) -> String {
    format!("-D{SECURITY_SYSTEM_PROPERTY_NAME}={mountpoint}/{SECURITY_FILE_NAME}")
}
