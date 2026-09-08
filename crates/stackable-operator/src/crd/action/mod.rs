//! Generic gated-action CRD: [`AgentRequest`](v1alpha1::AgentRequest).
//!
//! An `AgentRequest` is the out-of-process transport a product operator's scaling hook (or a
//! migration flow) uses to ask a per-cluster *agent* to perform a gated, product-specific action —
//! e.g. draining a Kafka broker's partitions before a scale-down. It is the companion to the
//! [`Scaler`](crate::crd::scaler) state machine: the hook cannot reach the product cluster itself
//! (it holds no product credentials), so it creates an `AgentRequest`, and the credentialed agent
//! reconciles it and reports progress back.
//!
//! Ownership is single-writer-per-subresource: the **operator writes `spec`**, the **agent is the
//! sole writer of `status`**. The envelope is deliberately generic (`context` is an untyped string
//! map) so the same CRD serves kafka/hdfs/nifi/trino; only the agent's executor is product-specific.

use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::versioned::versioned;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// A request for a per-cluster agent to perform a gated, product-specific action on behalf of
    /// an operator. The operator writes the spec; the agent is the sole writer of the status.
    #[versioned(crd(
        group = "platform.stackable.tech",
        status = AgentRequestStatus,
        doc = "The out-of-process transport a product operator's scaling hook uses to ask a per-cluster agent to perform a gated, product-specific action (e.g. broker drain). Operator writes spec; agent writes status.",
        namespaced
    ))]
    #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AgentRequestSpec {
        /// Reference to the product cluster this action targets. The namespace is this
        /// AgentRequest's own namespace (an AgentRequest is namespaced and co-located with its
        /// cluster), so only the name is needed.
        pub cluster_ref: ClusterRef,

        /// Product the target cluster runs, e.g. `kafka`.
        pub product: String,

        /// Role within the cluster the action applies to, e.g. `broker`.
        pub role: String,

        /// The kind of action the agent should perform.
        pub action_type: ActionType,

        /// Loose, per-action payload, e.g. `brokerIds: "4,5"`. Deliberately an untyped string map so
        /// the envelope stays generic across products (no product-specific fields on this CRD).
        #[serde(default)]
        pub context: BTreeMap<String, String>,

        /// The operator deletes this AgentRequest this many seconds after it reaches the terminal
        /// `Done` phase. Failed/Rejected requests are retained for inspection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ttl_seconds_after_finished: Option<u32>,
    }
}

#[cfg(test)]
impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::AgentRequestSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        crate::utils::yaml_from_str_singleton_map(indoc::indoc! {"
          - clusterRef:
              name: my-kafka
            product: kafka
            role: broker
            actionType: scaleDown
          - clusterRef:
              name: my-kafka
            product: kafka
            role: broker
            actionType: scaleDown
            context:
              brokerIds: '4,5'
            ttlSecondsAfterFinished: 3600
          - clusterRef:
              name: my-kafka
            product: kafka
            role: broker
            actionType: unregister
            context:
              brokerIds: '4,5'
            ttlSecondsAfterFinished: 3600
          - clusterRef:
              name: my-zookeeper
            product: zookeeper
            role: server
            actionType: migration
        "})
        .expect("Failed to parse AgentRequestSpec YAML")
    }
}

/// Reference to a product cluster CR in the same namespace as the AgentRequest.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterRef {
    /// Name of the product cluster CR. Namespace is inferred from the AgentRequest's namespace.
    pub name: String,
}

/// The kind of gated action requested.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionType {
    /// Drain a role's soon-to-be-removed pods before a scale-down (e.g. Kafka partition reassignment).
    ScaleDown,
    /// Remove already-decommissioned members from the cluster's membership registry *after* a
    /// scale-down, once their pods are gone (e.g. Kafka `UnregisterBroker`). The post-scale
    /// counterpart to `ScaleDown`.
    Unregister,
    /// A one-time breaking change during a version upgrade.
    BreakingUpgrade,
    /// A one-time data/config migration (e.g. ZooKeeper -> ZooKeeperless).
    Migration,
}

/// Status of an [`AgentRequest`](v1alpha1::AgentRequest) — written **solely by the product agent**.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestStatus {
    /// Coarse lifecycle phase the operator gates on.
    #[serde(default)]
    pub phase: AgentRequestPhase,

    /// Product-specific stage label for humans, e.g. `reassigning`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,

    /// Human-readable detail; most relevant for `Failed` / `Rejected`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Optional progress counts for slow-vs-stuck visibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<AgentRequestProgress>,
}

/// Coarse lifecycle phase of an [`AgentRequest`](v1alpha1::AgentRequest).
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, strum::Display)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum AgentRequestPhase {
    /// Created by the operator; the agent has not started yet.
    #[default]
    Pending,
    /// The agent is executing the action.
    InProgress,
    /// The action completed successfully — the operator may proceed (e.g. scale the STS down).
    Done,
    /// The action failed after retries; responsibility hands back to the operator.
    Failed,
    /// The request was malformed or not applicable; the agent refused it.
    Rejected,
}

/// Optional progress counts reported by the agent.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestProgress {
    /// Total units of work (e.g. partitions to reassign).
    pub total: u32,
    /// Units of work still remaining.
    pub remaining: u32,
}
