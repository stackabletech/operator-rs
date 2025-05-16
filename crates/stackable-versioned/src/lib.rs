//! This crate enables versioning of structs and enums through procedural
//! macros.
//!
//! Currently supported versioning schemes:
//!
//! - Kubernetes API versions (eg: `v1alpha1`, `v1beta1`, `v1`, `v2`), with
//!   optional support for generating CRDs.
//!
//! Support will be extended to SemVer versions, as well as custom version
//! formats in the future.
//!
//! See [`versioned`] for an in-depth usage guide and a list of supported
//! parameters.

use std::collections::HashMap;

use k8s_version::Version;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
// Re-export macro
pub use stackable_versioned_macros::*;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct CrdValues {
    /// List of values needed when downgrading to a particular version.
    pub downgrades: HashMap<Version, Vec<CrdValue>>,

    /// List of values needed when upgrading to a particular version.
    pub upgrades: HashMap<Version, Vec<CrdValue>>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct CrdValue {
    /// The name of the field of the custom resource this value is for.
    pub name: String,

    /// The value to be used when upgrading or downgrading the custom resource.
    #[schemars(schema_with = "raw_object_schema")]
    pub value: serde_yaml::Value,
}

// TODO (@Techassi): Think about where this should live. Basically this already exists in
// stackable-operator, but we cannot use it without depending on it which I would like to
// avoid.
fn raw_object_schema(_: &mut schemars::r#gen::SchemaGenerator) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        extensions: [(
            "x-kubernetes-preserve-unknown-fields".to_owned(),
            serde_json::Value::Bool(true),
        )]
        .into(),
        ..Default::default()
    })
}
