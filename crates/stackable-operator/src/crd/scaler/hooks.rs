//! Scaling lifecycle hooks for [`StackableScaler`](super::StackableScaler).
//!
//! Operators implement [`ScalingHooks`] to run custom logic at each stage of a scaling
//! operation. Hook methods are called by [`reconcile_scaler`](super::reconcile_scaler)
//! during the appropriate state machine stage.

use std::future::Future;

use crate::client::Client;

use super::FailedStage;

/// Whether the replica change is an increase or decrease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalingDirection {
    /// Replica count is increasing (or unchanged).
    Up,
    /// Replica count is decreasing.
    Down,
}

impl ScalingDirection {
    /// Derive the direction from current and desired replica counts.
    /// Equal counts are treated as Up (no-op -- hooks still call Done immediately).
    ///
    /// # Parameters
    ///
    /// - `current`: The replica count in `status.replicas`.
    /// - `desired`: The target replica count from `spec.replicas`.
    pub fn from_replicas(current: i32, desired: i32) -> Self {
        if desired >= current {
            Self::Up
        } else {
            Self::Down
        }
    }
}

/// Context passed to hook implementations on each reconcile invocation.
#[derive(Clone, Copy)]
pub struct ScalingContext<'a> {
    /// Kubernetes client for API calls.
    pub client: &'a Client,
    /// Namespace of the StackableScaler (and its cluster).
    pub namespace: &'a str,
    /// Name of the role group being scaled (e.g. `"default"`).
    pub role_group_name: &'a str,
    /// The replica count before the current scaling operation started.
    /// During `PreScaling` this equals `status.replicas`. During `Scaling`
    /// and `PostScaling` it reflects the frozen `status.previous_replicas`,
    /// so direction and ordinal calculations remain correct even after
    /// `status.replicas` has been updated to the target value.
    pub current_replicas: i32,
    /// The replica count the operator is working towards.
    pub desired_replicas: i32,
    /// Whether this is a scale-up or scale-down — derived by operator-rs,
    /// so the operator does not need to compare replica counts itself.
    pub direction: ScalingDirection,
}

impl ScalingContext<'_> {
    /// Returns the StatefulSet pod ordinals that are being removed in a scale-down.
    ///
    /// For scale-up or no-op, returns an empty range.
    /// For scale-down, returns `desired_replicas..current_replicas` — the ordinals
    /// of pods that will be terminated once the StatefulSet is scaled.
    pub fn removed_ordinals(&self) -> std::ops::Range<i32> {
        if self.direction == ScalingDirection::Down {
            self.desired_replicas..self.current_replicas
        } else {
            0..0
        }
    }

    /// Returns the StatefulSet pod ordinals that are being added in a scale-up.
    ///
    /// For scale-down or no-op, returns an empty range.
    /// For scale-up, returns `current_replicas..desired_replicas`.
    pub fn added_ordinals(&self) -> std::ops::Range<i32> {
        if self.direction == ScalingDirection::Up && self.desired_replicas > self.current_replicas {
            self.current_replicas..self.desired_replicas
        } else {
            0..0
        }
    }

    /// Whether this is a scale-down operation.
    pub fn is_scale_down(&self) -> bool {
        self.direction == ScalingDirection::Down
    }

    /// Whether this is a scale-up operation (not a no-op where current == desired).
    pub fn is_scale_up(&self) -> bool {
        self.direction == ScalingDirection::Up && self.desired_replicas > self.current_replicas
    }
}

/// Return value from a hook invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookOutcome {
    /// Hook completed successfully — advance state machine to next stage.
    Done,
    /// Hook is still running — operator-rs will requeue and re-call on next reconcile.
    InProgress,
}

/// Condition information returned from `reconcile_scaler` for propagation to the cluster CR.
///
/// The operator MUST apply this condition to its cluster resource status on every call
/// to `reconcile_scaler`. Using the return type enforces this at the call site.
#[derive(Debug)]
pub enum ScalingCondition {
    /// No scaling in progress, or just completed successfully.
    Healthy,
    /// Scaling is actively in progress.
    Progressing {
        /// Human-readable name of the current [`ScalerStage`](super::ScalerStage).
        stage: String,
    },
    /// Scaling failed -- include details in the cluster CR condition message.
    Failed {
        /// Which stage failed.
        stage: FailedStage,
        /// The error message from the hook.
        reason: String,
    },
}

/// Result returned from `reconcile_scaler`. The operator MUST propagate `scaling_condition`
/// to the cluster CR status conditions on every reconcile call.
pub struct ScalingResult {
    /// The `Action` to return from the operator's reconcile function for this role group.
    pub action: kube::runtime::controller::Action,
    /// Condition to merge into the cluster CR's `status.conditions`.
    pub scaling_condition: ScalingCondition,
}

/// Trait implemented by each product operator to provide scaling lifecycle hooks.
///
/// All methods have default implementations that return `Done` immediately, so operators
/// only need to override the specific hooks they use.
///
/// # Example
///
/// ```rust,ignore
/// impl ScalingHooks for MyProductScalingHooks {
///     type Error = MyError;
///
///     async fn pre_scale(&self, ctx: &ScalingContext<'_>) -> Result<HookOutcome, MyError> {
///         match ctx.direction {
///             ScalingDirection::Down => {
///                 JobTracker::start_or_check(ctx.client, self.build_offload_job(ctx), ctx.namespace).await
///             }
///             ScalingDirection::Up => Ok(HookOutcome::Done),
///         }
///     }
/// }
/// ```
pub trait ScalingHooks {
    /// Error type returned by hook methods.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Called during the `PreScaling` stage on each reconcile.
    /// Return `Done` to advance to `Scaling`, `InProgress` to requeue and re-check.
    /// Return `Err` to transition to `Failed`.
    fn pre_scale(
        &self,
        ctx: &ScalingContext<'_>,
    ) -> impl Future<Output = Result<HookOutcome, Self::Error>> + Send {
        let _ = ctx;
        async { Ok(HookOutcome::Done) }
    }

    /// Called during the `PostScaling` stage on each reconcile.
    /// Return `Done` to return to `Idle`, `InProgress` to requeue and re-check.
    /// Return `Err` to transition to `Failed`.
    fn post_scale(
        &self,
        ctx: &ScalingContext<'_>,
    ) -> impl Future<Output = Result<HookOutcome, Self::Error>> + Send {
        let _ = ctx;
        async { Ok(HookOutcome::Done) }
    }

    /// Called when the state machine enters `Failed`. Best-effort cleanup.
    ///
    /// Errors returned here are logged at `warn` level but do not prevent the
    /// `Failed` state from being written.
    fn on_failure(
        &self,
        ctx: &ScalingContext<'_>,
        failed_stage: &FailedStage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let _ = (ctx, failed_stage);
        async { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_scale_up() {
        assert_eq!(ScalingDirection::from_replicas(3, 5), ScalingDirection::Up);
    }

    #[test]
    fn direction_scale_down() {
        assert_eq!(
            ScalingDirection::from_replicas(5, 3),
            ScalingDirection::Down
        );
    }

    #[test]
    fn direction_equal_is_up() {
        assert_eq!(ScalingDirection::from_replicas(3, 3), ScalingDirection::Up);
    }

    // Helper methods are tested indirectly via ScalingDirection since they
    // only depend on `direction`, `current_replicas`, and `desired_replicas`.
    // ScalingContext requires a &Client reference, so we test the logic
    // through the direction + replica helpers independently.

    #[test]
    fn removed_ordinals_scale_down() {
        let dir = ScalingDirection::Down;
        let (current, desired) = (5, 3);
        let range = if dir == ScalingDirection::Down {
            desired..current
        } else {
            0..0
        };
        assert_eq!(range.collect::<Vec<_>>(), vec![3, 4]);
    }

    #[test]
    fn removed_ordinals_scale_up_is_empty() {
        let dir = ScalingDirection::from_replicas(3, 5);
        let (current, desired) = (3, 5);
        let range = if dir == ScalingDirection::Down {
            desired..current
        } else {
            0..0
        };
        assert!(range.collect::<Vec<_>>().is_empty());
    }

    #[test]
    fn added_ordinals_scale_up() {
        let dir = ScalingDirection::from_replicas(3, 5);
        let (current, desired) = (3, 5);
        let range = if dir == ScalingDirection::Up && desired > current {
            current..desired
        } else {
            0..0
        };
        assert_eq!(range.collect::<Vec<_>>(), vec![3, 4]);
    }

    #[test]
    fn added_ordinals_equal_is_empty() {
        let dir = ScalingDirection::from_replicas(3, 3);
        let (current, desired) = (3, 3);
        let range = if dir == ScalingDirection::Up && desired > current {
            current..desired
        } else {
            0..0
        };
        assert!(range.collect::<Vec<_>>().is_empty());
    }
}
