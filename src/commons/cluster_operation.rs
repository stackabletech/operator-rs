use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// [Cluster operations](DOCS_BASE_URL_PLACEHOLDER/concepts/operations/cluster_operations)
/// properties, allow stopping the product instance as well as pausing reconciliation.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOperation {
    /// Flag to stop the cluster. This means all deployed resources (e.g. Services, StatefulSets,
    /// ConfigMaps) are kept but all deployed Pods (e.g. replicas from a StatefulSet) are scaled to 0
    /// and therefore stopped and removed.
    /// If applied at the same time with `reconciliationPaused`, the latter will pause reconciliation
    /// and `stopped` will take no effect until `reconciliationPaused` is set to false or removed.
    #[serde(default)]
    pub stopped: bool,
    /// Flag to stop cluster reconciliation by the operator. This means that all changes in the
    /// custom resource spec are ignored until this flag is set to false or removed. The operator
    /// will however still watch the deployed resources at the time and update the custom resource
    /// status field.
    /// If applied at the same time with `stopped`, `reconciliationPaused` will take precedence over
    /// `stopped` and stop the reconciliation immediately.
    #[serde(default)]
    pub reconciliation_paused: bool,
}
