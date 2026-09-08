//! Stackable scaler CRD and reconciliation framework.
//!
//! This module provides [`Scaler`](v1alpha1::Scaler), a Kubernetes custom resource that exposes a
//! `/scale` subresource so that a `HorizontalPodAutoscaler` can manage replica counts for
//! Stackable cluster role groups instead of targeting a `StatefulSet` directly.
//!
//! # State machine
//!
//! A [`Scaler`](v1alpha1::Scaler) progresses through states tracked in [`ScalerState`]:
//!
//! ```text
//! Idle → PreScaling → Scaling → PostScaling → Idle
//!                 ↘        ↘           ↘
//!                          Failed
//! ```
//!
//! Operators provide lifecycle hooks via the [`ScalingHooks`] trait and call
//! [`reconcile_scaler`] on every reconcile loop iteration for the relevant role group.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::kvp::Annotation;
use crate::versioned::versioned;

mod builder;
mod cluster_resource_impl;
pub mod hooks;
mod hpa_builder;
pub mod job_tracker;
pub mod reconciler;
mod replicas_config;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    #[versioned(crd(
        group = "autoscaling.stackable.tech",
        status = ScalerStatus,
        scale(
            spec_replicas_path = ".spec.replicas",
            status_replicas_path = ".status.replicas",
            label_selector_path = ".status.selector"
        ),
        doc = "Controls the replica count of a Stackable component, integrating with the Kubernetes scale subresource to enable horizontal autoscaling.",
        namespaced
    ))]
    #[derive(Clone, Debug, PartialEq, Eq, CustomResource, Deserialize, Serialize, JsonSchema)]
    pub struct ScalerSpec {
        /// Desired replica count.
        ///
        /// Written by the horizontal pod autoscaling mechanism via the /scale subresource.
        ///
        /// NOTE: This and other replica fields)use a [`u16`] instead of a [`i32`] used by
        /// [`k8s_openapi`] types to force a non-negative replica count. All [`u16`]s can be
        /// converted losslessly to [`i32`]s where needed.
        ///
        /// Upstream issues:
        ///
        /// - <https://github.com/kubernetes/kubernetes/issues/105533>
        /// - <https://github.com/Arnavion/k8s-openapi/issues/136>
        pub replicas: u16,
    }
}

/// Status of a StackableScaler.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScalerStatus {
    /// The current total number of replicas targeted by the managed StatefulSet.
    ///
    /// Exposed via the `/scale` subresource for horizontal pod autoscaling consumption.
    pub replicas: u16,

    /// Label selector string for HPA pod counting. Written at `.status.selector`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,

    /// The current state of the scaler state machine.
    pub state: ScalerState,

    /// Timestamp indicating when the scaler state last transitioned.
    pub last_transition_time: Time,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, strum::Display)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum ScalerState {
    /// No scaling operation is in progress.
    Idle {},

    /// Running the `pre_scale` hook (e.g. data offload).
    PreScaling {},

    /// Waiting for the StatefulSet to converge to the new replica count.
    ///
    /// This stage additionally tracks the previous replica count to be able derive the direction
    /// of the scaling operation.
    Scaling { previous_replicas: u16 },

    /// Running the `post_scale` hook (e.g. cluster rebalance).
    ///
    /// This stage additionally tracks the previous replica count to be able derive the direction
    /// of the scaling operation.
    PostScaling { previous_replicas: u16 },

    /// A hook returned an error.
    ///
    /// The scaler stays here until the user applies the [`Annotation::autoscaling_retry`] annotation
    /// to trigger a reset to [`ScalerState::Idle`].
    Failed {
        /// Which stage produced the error.
        failed_in: FailedInState,

        /// Human-readable error message from the hook.
        reason: String,
    },
}

impl ScalerState {
    /// Returns `true` when a scaling operation is actively running
    /// (`PreScaling`, `Scaling`, or `PostScaling`).
    ///
    /// `Idle` and `Failed` are not considered active — the HPA is
    /// free to write `spec.replicas` in those states.
    pub fn is_scaling_in_progress(&self) -> bool {
        matches!(
            self,
            Self::PreScaling { .. } | Self::Scaling { .. } | Self::PostScaling { .. }
        )
    }
}

/// In which state the scaling operation failed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum FailedInState {
    /// The `pre_scale` hook returned an error.
    PreScaling,

    /// The StatefulSet failed to reach the desired replica count.
    Scaling,

    /// The `post_scale` hook returned an error.
    PostScaling,
}

/// Resolve the replica count for a StatefulSet, taking an optional
/// [`v1alpha1::Scaler`] into account.
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
/// - `scaler`: The [`v1alpha1::Scaler`] for this role group, if one exists. Only consulted
///   when `role_group_replicas` is `Some(0)`.
///
/// # Returns
///
/// The effective replica count, or `None` if the scaler has no status yet.
pub fn resolve_replicas(
    role_group_replicas: Option<i32>,
    scaler: Option<&v1alpha1::Scaler>,
) -> Option<i32> {
    match (role_group_replicas, scaler) {
        (Some(0), Some(s)) => s.status.as_ref().map(|st| i32::from(st.replicas)),
        (replicas, _) => replicas,
    }
}

pub use builder::{BuildScalerError, build_scaler};
pub use hooks::{
    HookOutcome, ScalingCondition, ScalingContext, ScalingDirection, ScalingHooks, ScalingResult,
};
pub use hpa_builder::{
    InitializeStatusError, build_hpa_from_user_spec, initialize_scaler_status, scale_target_ref,
};
pub use job_tracker::{JobTracker, JobTrackerError, job_name};
pub use reconciler::{Error as ReconcilerError, reconcile_scaler};
pub use replicas_config::{
    AutoConfig, HpaConfig, ReplicasConfig, ValidationError as ReplicasValidationError,
};

#[cfg(test)]
impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::ScalerSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        crate::utils::yaml_from_str_singleton_map(indoc::indoc! {"
          - replicas: 0
          - replicas: 1
          - replicas: 42
          - replicas: 65535
        "})
        .expect("Failed to parse ScalerSpec YAML")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_scaling_in_progress_true_for_active_states() {
        assert!(ScalerState::PreScaling {}.is_scaling_in_progress());
        assert!(
            ScalerState::Scaling {
                previous_replicas: 3
            }
            .is_scaling_in_progress()
        );
        assert!(
            ScalerState::PostScaling {
                previous_replicas: 3
            }
            .is_scaling_in_progress()
        );
    }

    #[test]
    fn is_scaling_in_progress_false_for_idle_and_failed() {
        assert!(!ScalerState::Idle {}.is_scaling_in_progress());
        assert!(
            !ScalerState::Failed {
                failed_in: FailedInState::PreScaling,
                reason: "err".to_string(),
            }
            .is_scaling_in_progress()
        );
    }

    #[test]
    fn scaler_state_idle_serializes() {
        let state = ScalerState::Idle {};
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["idle"], serde_json::json!({}));
    }

    #[test]
    fn scaler_state_failed_serializes() {
        let state = ScalerState::Failed {
            failed_in: FailedInState::PreScaling,
            reason: "timeout".to_string(),
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["failed"]["failedIn"], "PreScaling");
        assert_eq!(json["failed"]["reason"], "timeout");
    }

    #[test]
    fn spec_round_trips() {
        let spec = v1alpha1::ScalerSpec { replicas: 3 };
        let json = serde_json::to_string(&spec).unwrap();
        let back: v1alpha1::ScalerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    fn test_status(replicas: u16) -> ScalerStatus {
        ScalerStatus {
            replicas,
            selector: None,
            state: ScalerState::Idle {},
            last_transition_time: Time(k8s_openapi::jiff::Timestamp::now()),
        }
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
        let mut scaler = v1alpha1::Scaler::new("test", v1alpha1::ScalerSpec { replicas: 5 });
        scaler.status = Some(test_status(3));
        assert_eq!(resolve_replicas(Some(0), Some(&scaler)), Some(3));
    }

    #[test]
    fn resolve_replicas_nonzero_with_scaler_ignores_scaler() {
        // role_group.replicas != 0 → scaler is not active (validation webhook should prevent this,
        // but we defensively fall back to the role group value)
        let mut scaler = v1alpha1::Scaler::new("test", v1alpha1::ScalerSpec { replicas: 5 });
        scaler.status = Some(test_status(4));
        assert_eq!(resolve_replicas(Some(3), Some(&scaler)), Some(3));
    }

    #[test]
    fn resolve_replicas_zero_scaler_no_status_returns_none() {
        // Scaler exists but has no status yet (just created) → return None (don't set replicas)
        let scaler = v1alpha1::Scaler::new("test", v1alpha1::ScalerSpec { replicas: 5 });
        assert_eq!(resolve_replicas(Some(0), Some(&scaler)), None);
    }
}
