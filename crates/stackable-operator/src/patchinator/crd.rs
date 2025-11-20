use kube::api::DynamicObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::crds::raw_object_list_schema;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOverrides {
    /// A list of generic Kubernetes objects, which are merged onto the objects that the operator
    /// creates.
    ///
    // TODO: Add link to concepts page once it exists
    #[serde(default)]
    #[schemars(schema_with = "raw_object_list_schema")]
    pub object_overrides: Vec<DynamicObject>,
}
