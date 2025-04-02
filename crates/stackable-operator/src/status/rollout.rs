//! Tools for managing rollouts of Pod controllers (such as [`StatefulSet`]).

use std::borrow::Cow;

use k8s_openapi::api::apps::v1::StatefulSet;
use snafu::Snafu;

/// The reason for why a [`StatefulSet`] is still rolling out. Returned by [`check_statefulset_rollout_complete`].
#[derive(Debug, Snafu)]
#[snafu(module(outdated_statefulset))]
pub enum StatefulSetRolloutInProgress {
    /// Indicates that the latest version of the [`StatefulSet`] has not yet been observed by Kubernetes' StatefulSet controller.
    ///
    /// Kubernetes' controllers run asynchronously in the background, so this is expected when the `spec` has just been modified.
    #[snafu(display(
        "generation {current_generation:?} not yet observed by statefulset controller, last seen was {observed_generation:?}"
    ))]
    NotYetObserved {
        current_generation: Option<i64>,
        observed_generation: Option<i64>,
    },

    /// Indicates that outdated replicas still exist.
    #[snafu(display("only {updated_replicas} out of {total_replicas} are updated"))]
    HasOutdatedReplicas {
        total_replicas: i32,
        updated_replicas: i32,
    },
}

/// Checks whether all ReplicaSet replicas are up-to-date according to `sts.spec`.
///
/// "Success" here means that there are no replicas running an old version,
/// *not* that all updated replicas are available yet.
pub fn check_statefulset_rollout_complete(
    sts: &StatefulSet,
) -> Result<(), StatefulSetRolloutInProgress> {
    use outdated_statefulset::*;

    let status = sts.status.as_ref().map_or_else(Cow::default, Cow::Borrowed);

    let current_generation = sts.metadata.generation;
    let observed_generation = status.observed_generation;
    if current_generation != observed_generation {
        return NotYetObservedSnafu {
            current_generation,
            observed_generation,
        }
        .fail();
    }

    let total_replicas = status.replicas;
    let updated_replicas = status.updated_replicas.unwrap_or(0);
    if total_replicas != updated_replicas {
        return HasOutdatedReplicasSnafu {
            total_replicas,
            updated_replicas,
        }
        .fail();
    }

    Ok(())
}
