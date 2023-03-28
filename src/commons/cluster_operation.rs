use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOperation {
    #[serde(default)]
    pub stopped: bool,
    #[serde(default)]
    pub reconciliation_paused: bool,
}
