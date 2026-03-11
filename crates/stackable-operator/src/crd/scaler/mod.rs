//! Stackable scaler CRD and reconciliation framework.
//!
//! This module provides [`StackableScaler`], a Kubernetes custom resource that exposes a
//! `/scale` subresource so that a `HorizontalPodAutoscaler` can manage replica counts for
//! Stackable cluster role groups instead of targeting a `StatefulSet` directly.
//!
//! # State machine
//!
//! A [`StackableScaler`] progresses through stages tracked in [`ScalerStage`]:
//!
//! ```text
//! Idle → PreScaling → Scaling → PostScaling → Idle
//!                 ↘        ↘           ↘
//!                          Failed
//! ```
//!
//! Operators provide lifecycle hooks via the [`ScalingHooks`] trait and call
//! [`reconcile_scaler`] on every reconcile loop iteration for the relevant role group.

use std::borrow::Cow;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod hooks;
pub mod job_tracker;
pub mod reconciler;

/// A type-erased cluster reference used in StackableScaler.
/// Does not carry apiVersion — CRD versioning handles conversions.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownClusterRef {
    /// The Kubernetes kind of the target cluster resource (e.g. "NifiCluster").
    pub kind: String,
    /// The name of the target cluster resource within the same namespace.
    pub name: String,
}

/// Which stage of a scaling operation failed.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailedStage {
    /// The [`ScalingHooks::pre_scale`] hook returned an error.
    PreScaling,
    /// The StatefulSet failed to reach the desired replica count.
    Scaling,
    /// The [`ScalingHooks::post_scale`] hook returned an error.
    PostScaling,
}

/// The current stage of the scaling state machine.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "stage", content = "details", rename_all = "camelCase")]
pub enum ScalerStage {
    /// No scaling operation is in progress.
    Idle,
    /// Running the [`ScalingHooks::pre_scale`] hook (e.g. data offload).
    PreScaling,
    /// Waiting for the StatefulSet to converge to the new replica count.
    Scaling,
    /// Running the [`ScalingHooks::post_scale`] hook (e.g. cluster rebalance).
    PostScaling,
    /// A hook returned an error. The scaler stays here until manually reset.
    Failed {
        /// Which stage produced the error.
        #[serde(rename = "failedAt")]
        failed_at: FailedStage,
        /// Human-readable error message from the hook.
        reason: String,
    },
}

impl ScalerStage {
    /// Returns `true` when a scaling operation is actively running
    /// (`PreScaling`, `Scaling`, or `PostScaling`).
    ///
    /// `Idle` and `Failed` are not considered active — the HPA is
    /// free to write `spec.replicas` in those states.
    pub fn is_scaling_in_progress(&self) -> bool {
        matches!(self, Self::PreScaling | Self::Scaling | Self::PostScaling)
    }
}

/// Formats the stage name for logging and status messages.
///
/// The `Failed` variant only includes the failed stage, not the full reason string,
/// to keep log messages concise.
impl std::fmt::Display for ScalerStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::PreScaling => write!(f, "PreScaling"),
            Self::Scaling => write!(f, "Scaling"),
            Self::PostScaling => write!(f, "PostScaling"),
            Self::Failed { failed_at, .. } => write!(f, "Failed({failed_at:?})"),
        }
    }
}

/// Manual [`JsonSchema`] implementation because `schemars` does not support the
/// `#[serde(tag = "stage", content = "details")]` internally-tagged representation
/// used by this enum.
impl JsonSchema for ScalerStage {
    fn schema_name() -> Cow<'static, str> {
        "ScalerStage".into()
    }

    fn json_schema(generator: &mut schemars::generate::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["stage"],
            "properties": {
                "stage": {
                    "type": "string",
                    "enum": ["idle", "preScaling", "scaling", "postScaling", "failed"]
                },
                "details": {
                    "type": "object",
                    "properties": {
                        "failedAt": generator.subschema_for::<FailedStage>(),
                        "reason": { "type": "string" }
                    }
                }
            }
        })
    }
}

/// The current state of the scaler, including when it last changed.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalerState {
    /// The current stage of the scaler state machine.
    pub stage: ScalerStage,
    /// When this stage was entered.
    pub last_transition_time: Time,
}

/// Status of a StackableScaler.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackableScalerStatus {
    /// The replica count currently targeted by the managed StatefulSet. Exposed via the `/scale` subresource for HPA consumption.
    pub replicas: i32,
    /// Label selector string for HPA pod counting. Written at `.status.selector`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// The target replica count for the in-progress scaling operation. `None` when idle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_replicas: Option<i32>,
    /// The replica count when the current scaling operation started. `None` when idle.
    /// Used to derive [`ScalingDirection`] correctly across all stages, because
    /// `status.replicas` is overwritten to the target value during the `Scaling` stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_replicas: Option<i32>,
    /// The current state machine stage and its transition timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_state: Option<ScalerState>,
}

/// A StackableScaler exposes a /scale subresource so that a Kubernetes
/// HorizontalPodAutoscaler can target it instead of a StatefulSet directly.
#[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "autoscaling.stackable.tech",
    version = "v1alpha1",
    kind = "StackableScaler",
    namespaced,
    status = "StackableScalerStatus",
    scale = r#"{"specReplicasPath":".spec.replicas","statusReplicasPath":".status.replicas","labelSelectorPath":".status.selector"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct StackableScalerSpec {
    /// Desired replica count. Written by the HPA via the /scale subresource.
    /// Only takes effect when the referenced roleGroup has `replicas: 0`.
    pub replicas: i32,
    /// Reference to the Stackable cluster resource this scaler manages.
    pub cluster_ref: UnknownClusterRef,
    /// The role within the cluster (e.g. `nodes`).
    pub role: String,
    /// The role group within the role (e.g. `default`).
    pub role_group: String,
}

/// Resolve the replica count for a StatefulSet, taking an optional [`StackableScaler`] into account.
///
/// A scaler is only effective when `role_group_replicas` is `Some(0)` — this is the platform
/// convention that signals "externally managed replicas". In all other cases the role group
/// value is used unchanged.
///
/// Always call this instead of reading `role_group.replicas` directly when building a StatefulSet,
/// to ensure scaler-managed role groups are handled consistently.
///
/// # Parameters
///
/// - `role_group_replicas`: The replica count from the role group config. `Some(0)` signals
///   externally-managed replicas (the scaler's value is used). Any other value is returned unchanged.
/// - `scaler`: The [`StackableScaler`] for this role group, if one exists. Only consulted
///   when `role_group_replicas` is `Some(0)`.
///
/// # Returns
///
/// The effective replica count, or `None` if the scaler has no status yet.
pub fn resolve_replicas(
    role_group_replicas: Option<i32>,
    scaler: Option<&StackableScaler>,
) -> Option<i32> {
    match (role_group_replicas, scaler) {
        (Some(0), Some(s)) => s.status.as_ref().map(|st| st.replicas),
        (replicas, _) => replicas,
    }
}

pub use hooks::{
    HookOutcome, ScalingCondition, ScalingContext, ScalingDirection, ScalingHooks, ScalingResult,
};
pub use job_tracker::{JobTracker, JobTrackerError, job_name};
pub use reconciler::{Error as ReconcilerError, reconcile_scaler};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_scaling_in_progress_true_for_active_stages() {
        assert!(ScalerStage::PreScaling.is_scaling_in_progress());
        assert!(ScalerStage::Scaling.is_scaling_in_progress());
        assert!(ScalerStage::PostScaling.is_scaling_in_progress());
    }

    #[test]
    fn is_scaling_in_progress_false_for_idle_and_failed() {
        assert!(!ScalerStage::Idle.is_scaling_in_progress());
        assert!(
            !ScalerStage::Failed {
                failed_at: FailedStage::PreScaling,
                reason: "err".to_string(),
            }
            .is_scaling_in_progress()
        );
    }

    #[test]
    fn scaler_stage_idle_serializes() {
        let stage = ScalerStage::Idle;
        let json = serde_json::to_value(&stage).unwrap();
        assert_eq!(json["stage"], "idle");
    }

    #[test]
    fn scaler_stage_failed_serializes() {
        let stage = ScalerStage::Failed {
            failed_at: FailedStage::PreScaling,
            reason: "timeout".to_string(),
        };
        let json = serde_json::to_value(&stage).unwrap();
        assert_eq!(json["stage"], "failed");
        assert_eq!(json["details"]["failedAt"], "preScaling");
        assert_eq!(json["details"]["reason"], "timeout");
    }

    #[test]
    fn spec_round_trips() {
        let spec = StackableScalerSpec {
            replicas: 3,
            cluster_ref: UnknownClusterRef {
                kind: "NifiCluster".to_string(),
                name: "my-nifi".to_string(),
            },
            role: "nodes".to_string(),
            role_group: "default".to_string(),
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: StackableScalerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn resolve_replicas_no_scaler_uses_role_group() {
        assert_eq!(resolve_replicas(Some(3), None), Some(3));
    }

    #[test]
    fn resolve_replicas_none_role_group_no_scaler() {
        assert_eq!(resolve_replicas(None, None), None);
    }

    #[test]
    fn resolve_replicas_zero_with_scaler_uses_status() {
        let mut scaler = StackableScaler::new(
            "test",
            StackableScalerSpec {
                replicas: 5,
                cluster_ref: UnknownClusterRef {
                    kind: "NifiCluster".into(),
                    name: "n".into(),
                },
                role: "nodes".into(),
                role_group: "default".into(),
            },
        );
        scaler.status = Some(StackableScalerStatus {
            replicas: 3,
            ..Default::default()
        });
        assert_eq!(resolve_replicas(Some(0), Some(&scaler)), Some(3));
    }

    #[test]
    fn resolve_replicas_nonzero_with_scaler_ignores_scaler() {
        // role_group.replicas != 0 → scaler is not active (validation webhook should prevent this,
        // but we defensively fall back to the role group value)
        let mut scaler = StackableScaler::new(
            "test",
            StackableScalerSpec {
                replicas: 5,
                cluster_ref: UnknownClusterRef {
                    kind: "NifiCluster".into(),
                    name: "n".into(),
                },
                role: "nodes".into(),
                role_group: "default".into(),
            },
        );
        scaler.status = Some(StackableScalerStatus {
            replicas: 4,
            ..Default::default()
        });
        assert_eq!(resolve_replicas(Some(3), Some(&scaler)), Some(3));
    }

    #[test]
    fn resolve_replicas_zero_scaler_no_status_returns_none() {
        // Scaler exists but has no status yet (just created) → return None (don't set replicas)
        let scaler = StackableScaler::new(
            "test",
            StackableScalerSpec {
                replicas: 5,
                cluster_ref: UnknownClusterRef {
                    kind: "NifiCluster".into(),
                    name: "n".into(),
                },
                role: "nodes".into(),
                role_group: "default".into(),
            },
        );
        assert_eq!(resolve_replicas(Some(0), Some(&scaler)), None);
    }
}
