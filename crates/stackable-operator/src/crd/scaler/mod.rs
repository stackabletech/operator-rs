use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::kvp::Annotation;
use crate::versioned::versioned;

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
        /// - https://github.com/kubernetes/kubernetes/issues/105533
        /// - https://github.com/Arnavion/k8s-openapi/issues/136
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
