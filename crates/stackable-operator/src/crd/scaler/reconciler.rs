//! Reconciler for [`Scaler`](super::v1alpha1::Scaler) resources.
//!
//! The public entry point is [`reconcile_scaler`]. Operators call this on every reconcile
//! for a role group, and it drives the [`ScalerState`](super::ScalerState) state machine,
//! invokes [`ScalingHooks`] at the appropriate stages, and patches the scaler's status.

use std::time::Duration;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::Timestamp;
use kube::runtime::controller::Action;
use snafu::{OptionExt, ResultExt, Snafu};
use tracing::{debug, info, warn};

use crate::client::Client;
use crate::crd::scaler::hooks::{
    HookOutcome, ScalingCondition, ScalingContext, ScalingDirection, ScalingHooks, ScalingResult,
};
use crate::crd::scaler::{FailedInState, ScalerState, ScalerStatus, v1alpha1::Scaler};
use crate::kvp::Annotation;

/// Requeue interval when a hook returns [`HookOutcome::InProgress`].
const REQUEUE_HOOK_IN_PROGRESS: Duration = Duration::from_secs(10);
/// Requeue interval while waiting for the StatefulSet to converge.
const REQUEUE_SCALING: Duration = Duration::from_secs(5);

/// Errors returned by [`reconcile_scaler`].
#[derive(Debug, Snafu)]
pub enum Error {
    /// The Kubernetes status patch for the [`Scaler`] failed.
    #[snafu(display("failed to patch Scaler status"))]
    PatchStatus {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
    },
    /// Removing the retry annotation from the [`Scaler`] failed.
    #[snafu(display("failed to remove retry annotation from Scaler"))]
    RemoveRetryAnnotation {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
    },
    /// The [`Scaler`] is missing `.metadata.namespace`.
    #[snafu(display("Scaler object is missing namespace"))]
    ObjectHasNoNamespace,
}

/// Read the `previous_replicas` frozen inside the active state variant, if any.
///
/// Only [`ScalerState::Scaling`] and [`ScalerState::PostScaling`] carry it; all other states
/// return `None`.
fn previous_replicas_of(state: &ScalerState) -> Option<i32> {
    match state {
        ScalerState::Scaling { previous_replicas }
        | ScalerState::PostScaling { previous_replicas } => Some(i32::from(*previous_replicas)),
        _ => None,
    }
}

/// Compute the next state machine step from the current state and hook/stability inputs.
///
/// Hook outcomes are passed as closures so this function stays synchronous and
/// unit-testable without async infrastructure.
///
/// # Parameters
///
/// - `current`: The current [`ScalerState`].
/// - `current_replicas`: The replica count in `status.replicas`.
/// - `desired_replicas`: The target from `spec.replicas`.
/// - `pre_outcome`: Result of the `PreScaling` hook. Only called when in `PreScaling`.
/// - `post_outcome`: Result of the `PostScaling` hook. Only called when in `PostScaling`.
/// - `statefulset_stable`: Whether the StatefulSet has converged. Only relevant in `Scaling`.
fn next_state(
    current: &ScalerState,
    current_replicas: i32,
    desired_replicas: i32,
    pre_outcome: impl FnOnce() -> HookOutcome,
    post_outcome: impl FnOnce() -> HookOutcome,
    statefulset_stable: bool,
) -> NextState {
    match current {
        ScalerState::Idle {} => {
            if current_replicas == desired_replicas {
                NextState::NoChange
            } else {
                NextState::Transition(ScalerState::PreScaling {})
            }
        }
        ScalerState::PreScaling {} => match pre_outcome() {
            // Freeze the pre-scale replica count into the Scaling variant so direction can be
            // derived even after status.replicas is overwritten to the target value.
            HookOutcome::Done => NextState::Transition(ScalerState::Scaling {
                previous_replicas: clamp_to_u16(current_replicas),
            }),
            HookOutcome::InProgress => NextState::Requeue,
        },
        ScalerState::Scaling { previous_replicas } => {
            if statefulset_stable {
                NextState::Transition(ScalerState::PostScaling {
                    previous_replicas: *previous_replicas,
                })
            } else {
                NextState::Requeue
            }
        }
        ScalerState::PostScaling { .. } => match post_outcome() {
            HookOutcome::Done => NextState::Transition(ScalerState::Idle {}),
            HookOutcome::InProgress => NextState::Requeue,
        },
        ScalerState::Failed { .. } => NextState::NoChange,
    }
}

/// The outcome of [`next_state`]: what the reconciler should do.
#[derive(Debug, Eq, PartialEq)]
enum NextState {
    /// Nothing to do; wait for an external watch event.
    NoChange,
    /// Current state is not yet complete; requeue after a short interval.
    Requeue,
    /// Move to the given state and patch the scaler status.
    Transition(ScalerState),
}

/// Reconcile a [`Scaler`], advancing its state machine and invoking hooks.
///
/// Call this from your operator's reconcile function for every role group that has a
/// corresponding [`Scaler`]. The returned [`ScalingCondition`] MUST be applied
/// to the cluster CR's `status.conditions`.
///
/// # Parameters
///
/// - `scaler`: The [`Scaler`] resource. Must have `.metadata.namespace` set.
/// - `hooks`: The operator's [`ScalingHooks`] implementation.
/// - `client`: Kubernetes client for status patches and hook API calls.
/// - `statefulset_stable`: `true` when the managed StatefulSet has reached its target
///   replica count and all pods are ready.
/// - `selector`: Pod label selector string for this role group (e.g.
///   `"app=myproduct,roleGroup=default"`). Written into `status.selector` for HPA
///   pod counting. Must be stable across reconcile calls.
/// - `role_group_name`: The name of the role group being scaled (e.g. `"default"`).
///   Passed to hooks via [`ScalingContext`].
///
/// # Errors
///
/// Returns [`Error::PatchStatus`] if the status patch fails, or
/// [`Error::ObjectHasNoNamespace`] if the scaler has no namespace.
#[expect(clippy::too_many_lines)]
pub async fn reconcile_scaler<H>(
    scaler: &Scaler,
    hooks: &H,
    client: &Client,
    statefulset_stable: bool,
    selector: &str,
    role_group_name: &str,
) -> Result<ScalingResult, Error>
where
    H: ScalingHooks,
{
    let current_state = scaler
        .status
        .as_ref()
        .map_or(ScalerState::Idle {}, |s| s.state.clone());
    let current_replicas = scaler
        .status
        .as_ref()
        .map_or(0, |s| i32::from(s.replicas));
    let desired_replicas = i32::from(scaler.spec.replicas);
    let namespace = scaler
        .metadata
        .namespace
        .as_deref()
        .context(ObjectHasNoNamespaceSnafu)?;

    let scaler_name = scaler.metadata.name.as_deref().unwrap_or("<unknown>");

    debug!(
        scaler = scaler_name,
        %current_state,
        current_replicas,
        desired_replicas,
        statefulset_stable,
        "Reconciling Scaler"
    );

    // Recovery from Failed: if the user has set the retry annotation, strip it
    // and reset to Idle so the next reconcile starts a fresh scaling attempt.
    if matches!(current_state, ScalerState::Failed { .. }) {
        let retry_annotation = Annotation::autoscaling_retry(true);
        let retry_key = retry_annotation.key().to_string();
        let retry_value = retry_annotation.value().to_string();
        let has_retry = scaler
            .metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get(&retry_key))
            .is_some_and(|v| *v == retry_value);

        if has_retry {
            info!(
                scaler = scaler_name,
                "Retry annotation found on Failed scaler, resetting to Idle"
            );

            // Strip the annotation via merge patch (setting to null removes it)
            client
                .merge_patch(
                    scaler,
                    serde_json::json!({
                        "metadata": {
                            "annotations": {
                                retry_key: null
                            }
                        }
                    }),
                )
                .await
                .context(RemoveRetryAnnotationSnafu)?;

            // Reset status to Idle
            let idle_status = make_status(
                selector,
                ScalerState::Idle {},
                clamp_to_u16(current_replicas),
            );
            patch_status(client, scaler, idle_status)
                .await
                .context(PatchStatusSnafu)?;

            return Ok(ScalingResult {
                action: Action::requeue(REQUEUE_SCALING),
                scaling_condition: ScalingCondition::Healthy,
            });
        }
    }

    // When a scaling operation is in progress, use the frozen previous_replicas
    // (carried in the Scaling/PostScaling variant) to derive direction. status.replicas
    // is overwritten during the Scaling stage and would always yield Up otherwise.
    let direction_base = previous_replicas_of(&current_state).unwrap_or(current_replicas);
    let ctx = ScalingContext {
        client,
        namespace,
        role_group_name,
        current_replicas: direction_base,
        desired_replicas,
        direction: ScalingDirection::from_replicas(direction_base, desired_replicas),
    };

    // Run the hook for the current state if applicable, catching errors for Failed transition
    let pre_result = if matches!(current_state, ScalerState::PreScaling {}) {
        Some(hooks.pre_scale(&ctx).await)
    } else {
        None
    };

    let post_result = if matches!(current_state, ScalerState::PostScaling { .. }) {
        Some(hooks.post_scale(&ctx).await)
    } else {
        None
    };

    // Handle hook errors → Failed transition
    if let Some(Err(e)) = &pre_result {
        return handle_hook_failure(
            e,
            FailedInState::PreScaling,
            hooks,
            &ctx,
            scaler,
            selector,
            current_replicas,
        )
        .await;
    }

    if let Some(Err(e)) = &post_result {
        return handle_hook_failure(
            e,
            FailedInState::PostScaling,
            hooks,
            &ctx,
            scaler,
            selector,
            current_replicas,
        )
        .await;
    }

    let pre_outcome = pre_result
        .and_then(Result::ok)
        .unwrap_or(HookOutcome::Done);
    let post_outcome = post_result
        .and_then(Result::ok)
        .unwrap_or(HookOutcome::Done);

    let next = next_state(
        &current_state,
        current_replicas,
        desired_replicas,
        || pre_outcome.clone(),
        || post_outcome.clone(),
        statefulset_stable,
    );

    match next {
        NextState::NoChange => {
            debug!(
                scaler = scaler_name,
                %current_state,
                "No state change needed, awaiting external changes"
            );
            Ok(ScalingResult {
                action: Action::await_change(),
                scaling_condition: ScalingCondition::Healthy,
            })
        }
        NextState::Requeue => {
            let interval = if matches!(current_state, ScalerState::Scaling { .. }) {
                REQUEUE_SCALING
            } else {
                REQUEUE_HOOK_IN_PROGRESS
            };
            debug!(
                scaler = scaler_name,
                %current_state,
                requeue_after_secs = interval.as_secs(),
                "Requeuing, waiting for progress in current state"
            );
            Ok(ScalingResult {
                action: Action::requeue(interval),
                scaling_condition: ScalingCondition::Progressing {
                    stage: current_state.to_string(),
                },
            })
        }
        NextState::Transition(new_state) => {
            info!(
                scaler = scaler_name,
                %current_state,
                %new_state,
                current_replicas,
                desired_replicas,
                "Scaler transitioning state"
            );
            // When transitioning to Scaling, update status.replicas to the desired value
            // (this is what gets propagated to the StatefulSet). All other transitions keep
            // the current replica count.
            let new_replicas = if matches!(new_state, ScalerState::Scaling { .. }) {
                desired_replicas
            } else {
                current_replicas
            };
            let condition = match &new_state {
                ScalerState::Idle {} => ScalingCondition::Healthy,
                s => ScalingCondition::Progressing {
                    stage: s.to_string(),
                },
            };
            let new_status = make_status(selector, new_state, clamp_to_u16(new_replicas));
            patch_status(client, scaler, new_status)
                .await
                .context(PatchStatusSnafu)?;
            Ok(ScalingResult {
                action: Action::requeue(REQUEUE_SCALING),
                scaling_condition: condition,
            })
        }
    }
}

/// Transition the scaler to the `Failed` state after a hook error.
///
/// Patches the scaler status to `Failed` first, then calls [`ScalingHooks::on_failure`]
/// for best-effort cleanup. Writing the status before the cleanup hook guarantees that
/// a re-entrant reconcile sees `Failed` and will not invoke `on_failure` a second time.
///
/// If the cleanup hook itself fails, the status reason is updated to include the
/// cleanup error so it is visible via `kubectl describe` and the cluster CR condition.
///
/// # Parameters
///
/// - `error`: The hook error that caused the failure.
/// - `failed_in`: In which state (`PreScaling` or `PostScaling`) the error was produced.
/// - `hooks`: The operator's [`ScalingHooks`] implementation, used to call `on_failure`.
/// - `ctx`: The [`ScalingContext`] for the current reconcile, forwarded to `on_failure`.
///   Also provides the Kubernetes client for patching the scaler status.
/// - `scaler`: The [`Scaler`] resource being reconciled.
/// - `selector`: Pod label selector string written into `status.selector`.
/// - `current_replicas`: The replica count preserved in the `Failed` status so manual
///   recovery knows the original replica count.
async fn handle_hook_failure<H: ScalingHooks>(
    error: &H::Error,
    failed_in: FailedInState,
    hooks: &H,
    ctx: &ScalingContext<'_>,
    scaler: &Scaler,
    selector: &str,
    current_replicas: i32,
) -> Result<ScalingResult, Error> {
    let scaler_name = scaler.metadata.name.as_deref().unwrap_or("<unknown>");
    let hook_reason = error.to_string();
    warn!(
        scaler = scaler_name,
        failed_in = ?failed_in,
        error = %hook_reason,
        "Scaler hook failed, entering Failed state"
    );

    // Write Failed status BEFORE calling on_failure so that a subsequent reconcile
    // sees the Failed state and won't re-invoke on_failure.
    let new_status = make_status(
        selector,
        ScalerState::Failed {
            failed_in: failed_in.clone(),
            reason: hook_reason.clone(),
        },
        clamp_to_u16(current_replicas),
    );
    patch_status(ctx.client, scaler, new_status)
        .await
        .context(PatchStatusSnafu)?;

    // Run cleanup hook. If it fails, update the status reason so the failure is
    // visible via `kubectl describe` / the cluster CR condition.
    let final_reason = if let Err(on_failure_err) = hooks.on_failure(ctx, &failed_in).await {
        let reason_with_cleanup = format!("{hook_reason} (cleanup also failed: {on_failure_err})");
        warn!(
            scaler = scaler_name,
            error = %on_failure_err,
            failed_in = ?failed_in,
            "on_failure hook returned an error, updating status"
        );
        let updated_status = make_status(
            selector,
            ScalerState::Failed {
                failed_in: failed_in.clone(),
                reason: reason_with_cleanup.clone(),
            },
            clamp_to_u16(current_replicas),
        );
        // Best-effort update — if this patch also fails, the original Failed reason
        // is already persisted and the warn log captures the cleanup error.
        if let Err(patch_err) = patch_status(ctx.client, scaler, updated_status).await {
            warn!(
                scaler = scaler_name,
                error = %patch_err,
                "Failed to update status with cleanup error"
            );
        }
        reason_with_cleanup
    } else {
        hook_reason
    };

    Ok(ScalingResult {
        action: Action::await_change(),
        scaling_condition: ScalingCondition::Failed {
            state: failed_in,
            reason: final_reason,
        },
    })
}

/// Clamp an `i32` replica count to the non-negative `u16` range used by the CRD status.
///
/// Negative values (which should never occur) clamp to `0`; values above `u16::MAX`
/// clamp to `u16::MAX`. This is the outbound half of the `i32 <-> u16` reconciler seam.
fn clamp_to_u16(value: i32) -> u16 {
    value.clamp(0, i32::from(u16::MAX)) as u16
}

/// Construct a [`ScalerStatus`] with the given values and `last_transition_time` of now.
///
/// # Parameters
///
/// - `selector`: Pod label selector string for HPA pod counting.
/// - `state`: The new [`ScalerState`] to record in the status.
/// - `replicas`: The replica count to write into `status.replicas`.
fn make_status(selector: &str, state: ScalerState, replicas: u16) -> ScalerStatus {
    ScalerStatus {
        replicas,
        selector: Some(selector.to_string()),
        state,
        last_transition_time: Time(Timestamp::now()),
    }
}

/// Apply a server-side status patch to the [`Scaler`].
///
/// # Parameters
///
/// - `client`: Kubernetes client for the status patch operation.
/// - `scaler`: The [`Scaler`] resource whose status to update.
/// - `status`: The new [`ScalerStatus`] to apply.
async fn patch_status(
    client: &Client,
    scaler: &Scaler,
    status: ScalerStatus,
) -> Result<(), crate::client::Error> {
    client
        .apply_patch_status("stackable-operator", scaler, &status)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crd::scaler::hooks::HookOutcome;
    use crate::crd::scaler::{FailedInState, ScalerState};

    #[test]
    fn idle_transitions_to_prescaling_when_replicas_differ() {
        assert_eq!(
            next_state(
                &ScalerState::Idle {},
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextState::Transition(ScalerState::PreScaling {})
        );
    }

    #[test]
    fn idle_stays_idle_when_replicas_match() {
        assert_eq!(
            next_state(
                &ScalerState::Idle {},
                3,
                3,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextState::NoChange
        );
    }

    #[test]
    fn prescaling_advances_when_hook_done_freezing_previous_replicas() {
        assert_eq!(
            next_state(
                &ScalerState::PreScaling {},
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            // previous_replicas is frozen from current_replicas (3) at the PreScaling → Scaling edge
            NextState::Transition(ScalerState::Scaling {
                previous_replicas: 3
            })
        );
    }

    #[test]
    fn prescaling_requeues_when_hook_in_progress() {
        assert_eq!(
            next_state(
                &ScalerState::PreScaling {},
                3,
                5,
                || HookOutcome::InProgress,
                || HookOutcome::Done,
                false
            ),
            NextState::Requeue
        );
    }

    #[test]
    fn scaling_advances_when_statefulset_stable_carrying_previous_replicas() {
        assert_eq!(
            next_state(
                &ScalerState::Scaling {
                    previous_replicas: 3
                },
                5,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                true
            ),
            // previous_replicas carries through Scaling → PostScaling
            NextState::Transition(ScalerState::PostScaling {
                previous_replicas: 3
            })
        );
    }

    #[test]
    fn scaling_requeues_when_not_stable() {
        assert_eq!(
            next_state(
                &ScalerState::Scaling {
                    previous_replicas: 3
                },
                5,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextState::Requeue
        );
    }

    #[test]
    fn postscaling_returns_to_idle_when_hook_done() {
        assert_eq!(
            next_state(
                &ScalerState::PostScaling {
                    previous_replicas: 3
                },
                5,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                true
            ),
            NextState::Transition(ScalerState::Idle {})
        );
    }

    #[test]
    fn postscaling_requeues_when_hook_in_progress() {
        assert_eq!(
            next_state(
                &ScalerState::PostScaling {
                    previous_replicas: 3
                },
                5,
                5,
                || HookOutcome::Done,
                || HookOutcome::InProgress,
                true
            ),
            NextState::Requeue
        );
    }

    #[test]
    fn failed_stays_failed() {
        let failed = ScalerState::Failed {
            failed_in: FailedInState::PreScaling,
            reason: "err".to_string(),
        };
        assert_eq!(
            next_state(
                &failed,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextState::NoChange
        );
    }

    #[test]
    fn previous_replicas_read_from_scaling_variant() {
        assert_eq!(
            previous_replicas_of(&ScalerState::Scaling {
                previous_replicas: 7
            }),
            Some(7)
        );
        assert_eq!(
            previous_replicas_of(&ScalerState::PostScaling {
                previous_replicas: 2
            }),
            Some(2)
        );
        assert_eq!(previous_replicas_of(&ScalerState::Idle {}), None);
        assert_eq!(previous_replicas_of(&ScalerState::PreScaling {}), None);
    }

    #[test]
    fn clamp_to_u16_bounds() {
        assert_eq!(clamp_to_u16(-5), 0);
        assert_eq!(clamp_to_u16(0), 0);
        assert_eq!(clamp_to_u16(42), 42);
        assert_eq!(clamp_to_u16(70000), u16::MAX);
    }
}
