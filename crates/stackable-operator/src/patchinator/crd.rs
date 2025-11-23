use kube::api::DynamicObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::utils::crds::raw_object_list_schema;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOverrides {
    /// A list of generic Kubernetes objects, which are merged onto the objects that the operator
    /// creates.
    ///
    /// List entries are arbitrary YAML objects, which need to be valid Kubernetes objects.
    ///
    /// Read the [Object overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#object-overrides)
    /// for more information.
    #[serde(default)]
    #[schemars(schema_with = "raw_object_list_schema")]
    pub object_overrides: Vec<DynamicObject>,
}
