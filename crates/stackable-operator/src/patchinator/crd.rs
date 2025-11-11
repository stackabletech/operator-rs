use kube::api::DynamicObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::crds::raw_object_list_schema;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOverrides {
    #[serde(default)]
    #[schemars(schema_with = "raw_object_list_schema")]
    pub object_overrides: Vec<DynamicObject>,
}
