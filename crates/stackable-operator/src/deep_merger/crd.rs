use std::collections::HashSet;

use k8s_openapi::DeepMerge;
use kube::api::DynamicObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::apply_deep_merge;
use crate::utils::crds::raw_object_list_schema;

#[derive(Clone, Debug, Deserialize, Default, JsonSchema, Serialize, PartialEq)]
pub struct ObjectOverrides(
    /// A list of generic Kubernetes objects, which are merged into the objects that the operator
    /// creates.
    ///
    /// List entries are arbitrary YAML objects, which need to be valid Kubernetes objects.
    ///
    /// Read the [Object overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#object-overrides)
    /// for more information.
    //
    // Remember to use `#[serde(default)]` when including this into a CRD!
    #[schemars(schema_with = "raw_object_list_schema")]
    Vec<DynamicObject>,
);

impl ObjectOverrides {
    /// Takes an arbitrary Kubernetes object (`base`) and applies the configured list of deep merges
    /// to it.
    ///
    /// Merges are only applied to objects that have the same apiVersion, kind, name
    /// and namespace.
    ///
    /// Returns the indices of the entries that matched `base` and were therefore merged into it.
    pub fn apply_to<R>(&self, base: &mut R) -> Result<Vec<usize>, super::Error>
    where
        R: kube::Resource<DynamicType = ()> + DeepMerge + DeserializeOwned,
    {
        let mut matched_indices = Vec::new();

        for (index, object_override) in self.0.iter().enumerate() {
            if apply_deep_merge(base, object_override)? {
                matched_indices.push(index);
            }
        }

        Ok(matched_indices)
    }

    /// Returns all entries (and their index) that are not contained in `matched_indices`.
    pub fn unmatched<'a>(
        &'a self,
        matched_indices: &'a HashSet<usize>,
    ) -> impl Iterator<Item = (usize, &'a DynamicObject)> {
        self.0
            .iter()
            .enumerate()
            .filter(move |(index, _)| !matched_indices.contains(index))
    }
}
