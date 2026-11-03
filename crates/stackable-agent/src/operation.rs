use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// Operations on this resource in the product, independent of the `clusterOperation` of the
    /// product cluster.
    #[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResourceOperation {
        /// Stops the agent from changing the resource in the product. The status is still updated.
        #[serde(default)]
        pub reconciliation_paused: bool,
    }
}

#[cfg(test)]
mod tests {
    use super::v1alpha1::ResourceOperation;

    #[test]
    fn not_paused_by_default() {
        let operation: ResourceOperation =
            serde_json::from_str("{}").expect("the operation should be valid");

        assert!(!operation.reconciliation_paused);
    }

    #[test]
    fn deserialize_paused() {
        let operation: ResourceOperation =
            serde_json::from_str(r#"{"reconciliationPaused": true}"#)
                .expect("the operation should be valid");

        assert!(operation.reconciliation_paused);
    }
}
