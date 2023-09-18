use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pdb {
    /// Wether a PodDisruptionBudget should be written out for this role.
    /// Disabling this enables you to specify you own - custom - one.
    /// Defaults to true.
    #[serde(default = "default_pdb_enabled")]
    pub enabled: bool,
    /// The number of Pods that are allowed to be down because of voluntary disruptions.
    /// If you don't explicitly set this, the operator will use a sane default based
    /// upon knowledge about the individual product.
    pub max_unavailable: Option<u16>,
}

fn default_pdb_enabled() -> bool {
    true
}

impl Default for Pdb {
    fn default() -> Self {
        Self {
            enabled: true,
            max_unavailable: None,
        }
    }
}
