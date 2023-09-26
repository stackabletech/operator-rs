use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdbConfig {
    /// Whether a PodDisruptionBudget should be written out for this role.
    /// Disabling this enables you to specify your own - custom - one.
    /// Defaults to true.
    #[serde(default = "default_pdb_enabled")]
    pub enabled: bool,

    /// The number of Pods that are allowed to be down because of voluntary disruptions.
    /// If you don't explicitly set this, the operator will use a sane default based
    /// upon knowledge about the individual product.
    #[serde(default)]
    pub max_unavailable: Option<u16>,
}

fn default_pdb_enabled() -> bool {
    true
}

impl Default for PdbConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_unavailable: None,
        }
    }
}
