use k8s_openapi::DeepMerge;
use kube::api::DynamicObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::apply_deep_merge;
use crate::utils::crds::raw_object_list_schema;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOverrides {
    /// A list of generic Kubernetes objects, which are merged on the objects that the operator
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

impl ObjectOverrides {
    /// Takes an arbitrary Kubernetes object (`base`) and applies the configured list of deep merges
    /// to it.
    ///
    /// Merges are only applied to objects that have the same apiVersion, kind, name
    /// and namespace.
    pub fn apply_to<R>(&self, base: &mut R) -> Result<(), super::Error>
    where
        R: kube::Resource<DynamicType = ()> + DeepMerge + DeserializeOwned,
    {
        for object_override in &self.object_overrides {
            apply_deep_merge(base, object_override)?;
        }
        Ok(())
    }
}
