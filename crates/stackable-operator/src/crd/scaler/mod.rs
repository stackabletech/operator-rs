use std::borrow::Cow;

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
    #[derive(Clone, Debug, PartialEq, CustomResource, Deserialize, Serialize, JsonSchema)]
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
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
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

#[derive(Clone, Debug, Deserialize, Serialize, strum::Display)]
#[serde(
    tag = "state",
    content = "details",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[strum(serialize_all = "camelCase")]
pub enum ScalerState {
    /// No scaling operation is in progress.
    Idle,

    /// Running the `pre_scale` hook (e.g. data offload).
    PreScaling,

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

impl JsonSchema for ScalerState {
    fn schema_name() -> Cow<'static, str> {
        "ScalerState".into()
    }

    fn json_schema(generator: &mut schemars::generate::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "required": ["state"],
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["idle", "preScaling", "scaling", "postScaling", "failed"]
                },
                "details": {
                    "type": "object",
                    "properties": {
                        "failedIn": generator.subschema_for::<FailedInState>(),
                        "previous_replicas": {
                            "type": "uint16",
                            "minimum": u16::MIN,
                            "maximum": u16::MAX
                        },
                        "reason": { "type": "string" }
                    }
                }
            }
        })
    }
}

/// Which stage of a scaling operation failed.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FailedInState {
    /// The `pre_scale` hook returned an error.
    PreScaling,

    /// The StatefulSet failed to reach the desired replica count.
    Scaling,

    /// The `post_scale` hook returned an error.
    PostScaling,
}
