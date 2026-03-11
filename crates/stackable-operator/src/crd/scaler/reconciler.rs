//! Reconciler for [`StackableScaler`](super::StackableScaler) resources.
//!
//! The public entry point is [`reconcile_scaler`]. Operators call this on every reconcile
//! for a role group, and it drives the [`ScalerStage`](super::ScalerStage) state machine,
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
use crate::crd::scaler::{
    FailedStage, ScalerStage, ScalerState, StackableScaler, StackableScalerStatus,
};

/// Requeue interval when a hook returns [`HookOutcome::InProgress`].
const REQUEUE_HOOK_IN_PROGRESS: Duration = Duration::from_secs(10);
/// Requeue interval while waiting for the StatefulSet to converge.
const REQUEUE_SCALING: Duration = Duration::from_secs(5);

/// Errors returned by [`reconcile_scaler`].
#[derive(Debug, Snafu)]
pub enum Error {
    /// The Kubernetes status patch for the [`StackableScaler`] failed.
    #[snafu(display("failed to patch StackableScaler status"))]
    PatchStatus {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
    },
    /// The [`StackableScaler`] is missing `.metadata.namespace`.
    #[snafu(display("StackableScaler object is missing namespace"))]
    ObjectHasNoNamespace,
}

/// Compute the next state machine step from the current stage and hook/stability inputs.
///
/// Hook outcomes are passed as closures so this function stays synchronous and
/// unit-testable without async infrastructure.
///
/// # Parameters
///
/// - `current`: The current [`ScalerStage`].
/// - `current_replicas`: The replica count in `status.replicas`.
/// - `desired_replicas`: The target from `spec.replicas`.
/// - `pre_outcome`: Result of the `PreScaling` hook. Only called when in `PreScaling`.
/// - `post_outcome`: Result of the `PostScaling` hook. Only called when in `PostScaling`.
/// - `statefulset_stable`: Whether the StatefulSet has converged. Only relevant in `Scaling`.
fn next_stage(
    current: &ScalerStage,
    current_replicas: i32,
    desired_replicas: i32,
    pre_outcome: impl FnOnce() -> HookOutcome,
    post_outcome: impl FnOnce() -> HookOutcome,
    statefulset_stable: bool,
) -> NextStage {
    match current {
        ScalerStage::Idle => {
            if current_replicas != desired_replicas {
                NextStage::Transition(ScalerStage::PreScaling)
            } else {
                NextStage::NoChange
            }
        }
        ScalerStage::PreScaling => match pre_outcome() {
            HookOutcome::Done => NextStage::Transition(ScalerStage::Scaling),
            HookOutcome::InProgress => NextStage::Requeue,
        },
        ScalerStage::Scaling => {
            if statefulset_stable {
                NextStage::Transition(ScalerStage::PostScaling)
            } else {
                NextStage::Requeue
            }
        }
        ScalerStage::PostScaling => match post_outcome() {
            HookOutcome::Done => NextStage::Transition(ScalerStage::Idle),
            HookOutcome::InProgress => NextStage::Requeue,
        },
        ScalerStage::Failed { .. } => NextStage::NoChange,
    }
}

/// The outcome of [`next_stage`]: what the reconciler should do.
#[derive(Debug, Eq, PartialEq)]
enum NextStage {
    /// Nothing to do; wait for an external watch event.
    NoChange,
    /// Current stage is not yet complete; requeue after a short interval.
    Requeue,
    /// Move to the given stage and patch the scaler status.
    Transition(ScalerStage),
}

/// Reconcile a [`StackableScaler`], advancing its state machine and invoking hooks.
///
/// Call this from your operator's reconcile function for every role group that has a
/// corresponding [`StackableScaler`]. The returned [`ScalingCondition`] MUST be applied
/// to the cluster CR's `status.conditions`.
///
/// # Parameters
///
/// - `scaler`: The [`StackableScaler`] resource. Must have `.metadata.namespace` set.
/// - `hooks`: The operator's [`ScalingHooks`] implementation.
/// - `client`: Kubernetes client for status patches and hook API calls.
/// - `statefulset_stable`: `true` when the managed StatefulSet has reached its target
///   replica count and all pods are ready.
/// - `selector`: Pod label selector string for this role group (e.g.
///   `"app=myproduct,roleGroup=default"`). Written into `status.selector` for HPA
///   pod counting. Must be stable across reconcile calls.
///
/// # Errors
///
/// Returns [`Error::PatchStatus`] if the status patch fails, or
/// [`Error::ObjectHasNoNamespace`] if the scaler has no namespace.
pub async fn reconcile_scaler<H>(
    scaler: &StackableScaler,
    hooks: &H,
    client: &Client,
    statefulset_stable: bool,
    selector: &str,
) -> Result<ScalingResult, Error>
where
    H: ScalingHooks,
{
    let default_status = StackableScalerStatus::default();
    let status = scaler.status.as_ref().unwrap_or(&default_status);
    let current_stage = status
        .current_state
        .as_ref()
        .map(|s| s.stage.clone())
        .unwrap_or(ScalerStage::Idle);
    let current_replicas = status.replicas;
    let desired_replicas = scaler.spec.replicas;
    let namespace = scaler
        .metadata
        .namespace
        .as_deref()
        .context(ObjectHasNoNamespaceSnafu)?;

    debug!(
        scaler = scaler.metadata.name.as_deref().unwrap_or("<unknown>"),
        %current_stage,
        current_replicas,
        desired_replicas,
        statefulset_stable,
        "Reconciling StackableScaler"
    );

    // When a scaling operation is in progress, use the frozen previous_replicas
    // to derive direction. status.replicas is overwritten during the Scaling stage
    // and would always yield Up for the remainder of the operation.
    let direction_base = status.previous_replicas.unwrap_or(current_replicas);
    let ctx = ScalingContext {
        client,
        namespace,
        role_group_name: &scaler.spec.role_group,
        current_replicas: direction_base,
        desired_replicas,
        direction: ScalingDirection::from_replicas(direction_base, desired_replicas),
    };

    // Run the hook for the current stage if applicable, catching errors for Failed transition
    let pre_result = if matches!(current_stage, ScalerStage::PreScaling) {
        Some(hooks.pre_scale(&ctx).await)
    } else {
        None
    };

    let post_result = if matches!(current_stage, ScalerStage::PostScaling) {
        Some(hooks.post_scale(&ctx).await)
    } else {
        None
    };

    // Handle hook errors → Failed transition
    if let Some(Err(e)) = &pre_result {
        return handle_hook_failure(
            e,
            FailedStage::PreScaling,
            hooks,
            &ctx,
            scaler,
            selector,
            status,
        )
        .await;
    }

    if let Some(Err(e)) = &post_result {
        return handle_hook_failure(
            e,
            FailedStage::PostScaling,
            hooks,
            &ctx,
            scaler,
            selector,
            status,
        )
        .await;
    }

    let pre_outcome = pre_result.and_then(|r| r.ok()).unwrap_or(HookOutcome::Done);
    let post_outcome = post_result
        .and_then(|r| r.ok())
        .unwrap_or(HookOutcome::Done);

    let next = next_stage(
        &current_stage,
        current_replicas,
        desired_replicas,
        || pre_outcome.clone(),
        || post_outcome.clone(),
        statefulset_stable,
    );

    match next {
        NextStage::NoChange => {
            debug!(
                scaler = scaler.metadata.name.as_deref().unwrap_or("<unknown>"),
                %current_stage,
                "No stage change needed, awaiting external changes"
            );
            Ok(ScalingResult {
                action: Action::await_change(),
                scaling_condition: ScalingCondition::Healthy,
            })
        }
        NextStage::Requeue => {
            let interval = if matches!(current_stage, ScalerStage::Scaling) {
                REQUEUE_SCALING
            } else {
                REQUEUE_HOOK_IN_PROGRESS
            };
            debug!(
                scaler = scaler.metadata.name.as_deref().unwrap_or("<unknown>"),
                %current_stage,
                requeue_after_secs = interval.as_secs(),
                "Requeuing, waiting for progress in current stage"
            );
            Ok(ScalingResult {
                action: Action::requeue(interval),
                scaling_condition: ScalingCondition::Progressing {
                    stage: format!("{current_stage}"),
                },
            })
        }
        NextStage::Transition(new_stage) => {
            info!(
                scaler = scaler.metadata.name.as_deref().unwrap_or("<unknown>"),
                %current_stage,
                %new_stage,
                current_replicas,
                desired_replicas,
                "StackableScaler transitioning stage"
            );
            // When transitioning to Scaling, update status.replicas to desired
            // (this is what gets propagated to the StatefulSet)
            // When completing PostScaling → Idle, clear desiredReplicas
            let new_replicas = if matches!(new_stage, ScalerStage::Scaling) {
                desired_replicas
            } else {
                current_replicas
            };
            let new_desired = match new_stage {
                ScalerStage::Idle => None,
                ScalerStage::PreScaling => Some(desired_replicas),
                _ => status.desired_replicas,
            };
            let new_previous = match new_stage {
                ScalerStage::PreScaling => Some(current_replicas),
                ScalerStage::Idle => None,
                _ => status.previous_replicas,
            };
            let condition = match &new_stage {
                ScalerStage::Idle => ScalingCondition::Healthy,
                s => ScalingCondition::Progressing {
                    stage: format!("{s}"),
                },
            };
            let new_status =
                make_status(selector, new_stage, new_replicas, new_desired, new_previous);
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
/// - `failed_stage`: Which stage (`PreScaling` or `PostScaling`) produced the error.
/// - `hooks`: The operator's [`ScalingHooks`] implementation, used to call `on_failure`.
/// - `ctx`: The [`ScalingContext`] for the current reconcile, forwarded to `on_failure`.
///   Also provides the Kubernetes client for patching the scaler status.
/// - `scaler`: The [`StackableScaler`] resource being reconciled.
/// - `selector`: Pod label selector string written into `status.selector`.
/// - `status`: The current scaler status, preserved in the `Failed` status so manual
///   recovery knows the original replica counts.
async fn handle_hook_failure<H: ScalingHooks>(
    error: &H::Error,
    failed_stage: FailedStage,
    hooks: &H,
    ctx: &ScalingContext<'_>,
    scaler: &StackableScaler,
    selector: &str,
    status: &StackableScalerStatus,
) -> Result<ScalingResult, Error> {
    let scaler_name = scaler.metadata.name.as_deref().unwrap_or("<unknown>");
    let hook_reason = error.to_string();
    warn!(
        scaler = scaler_name,
        failed_at = ?failed_stage,
        error = %hook_reason,
        "StackableScaler hook failed, entering Failed state"
    );

    // Write Failed status BEFORE calling on_failure so that a subsequent reconcile
    // sees the Failed stage and won't re-invoke on_failure.
    let new_status = make_status(
        selector,
        ScalerStage::Failed {
            failed_at: failed_stage.clone(),
            reason: hook_reason.clone(),
        },
        status.replicas,
        status.desired_replicas,
        status.previous_replicas,
    );
    patch_status(ctx.client, scaler, new_status)
        .await
        .context(PatchStatusSnafu)?;

    // Run cleanup hook. If it fails, update the status reason so the failure is
    // visible via `kubectl describe` / the cluster CR condition.
    let final_reason = if let Err(on_failure_err) = hooks.on_failure(ctx, &failed_stage).await {
        let reason_with_cleanup = format!(
            "{hook_reason} (cleanup also failed: {on_failure_err})"
        );
        warn!(
            scaler = scaler_name,
            error = %on_failure_err,
            failed_at = ?failed_stage,
            "on_failure hook returned an error, updating status"
        );
        let updated_status = make_status(
            selector,
            ScalerStage::Failed {
                failed_at: failed_stage.clone(),
                reason: reason_with_cleanup.clone(),
            },
            status.replicas,
            status.desired_replicas,
            status.previous_replicas,
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
            stage: failed_stage,
            reason: final_reason,
        },
    })
}

/// Construct a [`StackableScalerStatus`] with the given values and `last_transition_time` of now.
///
/// # Parameters
///
/// - `selector`: Pod label selector string for HPA pod counting.
/// - `stage`: The new [`ScalerStage`] to record in the status.
/// - `replicas`: The replica count to write into `status.replicas`.
/// - `desired_replicas`: The in-flight target, or `None` when returning to `Idle`.
/// - `previous_replicas`: The replica count before the scaling operation started,
///   or `None` when returning to `Idle`.
fn make_status(
    selector: &str,
    stage: ScalerStage,
    replicas: i32,
    desired_replicas: Option<i32>,
    previous_replicas: Option<i32>,
) -> StackableScalerStatus {
    StackableScalerStatus {
        replicas,
        selector: Some(selector.to_string()),
        desired_replicas,
        previous_replicas,
        current_state: Some(ScalerState {
            stage,
            last_transition_time: Time(Timestamp::now()),
        }),
    }
}

/// Apply a server-side status patch to the [`StackableScaler`].
///
/// # Parameters
///
/// - `client`: Kubernetes client for the status patch operation.
/// - `scaler`: The [`StackableScaler`] resource whose status to update.
/// - `status`: The new [`StackableScalerStatus`] to apply.
async fn patch_status(
    client: &Client,
    scaler: &StackableScaler,
    status: StackableScalerStatus,
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
    use crate::crd::scaler::{FailedStage, ScalerStage};

    #[test]
    fn idle_transitions_to_prescaling_when_replicas_differ() {
        assert_eq!(
            next_stage(
                &ScalerStage::Idle,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextStage::Transition(ScalerStage::PreScaling)
        );
    }

    #[test]
    fn idle_stays_idle_when_replicas_match() {
        assert_eq!(
            next_stage(
                &ScalerStage::Idle,
                3,
                3,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextStage::NoChange
        );
    }

    #[test]
    fn prescaling_advances_when_hook_done() {
        assert_eq!(
            next_stage(
                &ScalerStage::PreScaling,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextStage::Transition(ScalerStage::Scaling)
        );
    }

    #[test]
    fn prescaling_requeues_when_hook_in_progress() {
        assert_eq!(
            next_stage(
                &ScalerStage::PreScaling,
                3,
                5,
                || HookOutcome::InProgress,
                || HookOutcome::Done,
                false
            ),
            NextStage::Requeue
        );
    }

    #[test]
    fn scaling_advances_when_statefulset_stable() {
        assert_eq!(
            next_stage(
                &ScalerStage::Scaling,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                true
            ),
            NextStage::Transition(ScalerStage::PostScaling)
        );
    }

    #[test]
    fn scaling_requeues_when_not_stable() {
        assert_eq!(
            next_stage(
                &ScalerStage::Scaling,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextStage::Requeue
        );
    }

    #[test]
    fn postscaling_returns_to_idle_when_hook_done() {
        assert_eq!(
            next_stage(
                &ScalerStage::PostScaling,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                true
            ),
            NextStage::Transition(ScalerStage::Idle)
        );
    }

    #[test]
    fn postscaling_requeues_when_hook_in_progress() {
        assert_eq!(
            next_stage(
                &ScalerStage::PostScaling,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::InProgress,
                true
            ),
            NextStage::Requeue
        );
    }

    #[test]
    fn failed_stays_failed() {
        let failed = ScalerStage::Failed {
            failed_at: FailedStage::PreScaling,
            reason: "err".to_string(),
        };
        assert_eq!(
            next_stage(
                &failed,
                3,
                5,
                || HookOutcome::Done,
                || HookOutcome::Done,
                false
            ),
            NextStage::NoChange
        );
    }
}
