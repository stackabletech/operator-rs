use std::fmt::Display;

use educe::Educe;
use kube::{Resource, runtime::reflector::ObjectRef};

/// A reference to a named role group of a given cluster object
#[derive(Educe)]
#[educe(Clone, Debug)]
pub struct RoleGroupRef<K: Resource> {
    pub cluster: ObjectRef<K>,
    pub role: String,
    pub role_group: String,
}

impl<K: Resource> RoleGroupRef<K> {
    pub fn object_name(&self) -> String {
        format!("{}-{}-{}", self.cluster.name, self.role, self.role_group)
    }

    /// Returns the service name used by rolegroups for cluster internal communication only.
    ///
    /// The internal use of of this service name is indicated by the `-headless` suffix.
    /// This service should not be used for communication to external services or clients
    /// and also should not be used to export metrics (like Prometheus). Metrics should be
    /// instead exposed via a dedicated service. Use [`Self::rolegroup_metrics_service_name`]
    /// instead.
    pub fn rolegroup_headless_service_name(&self) -> String {
        format!("{name}-headless", name = self.object_name())
    }

    /// Returns the service name used by rolegroups to expose metrics (like Prometheus).
    ///
    /// The use for metrics only is indicated by the `-metrics` suffix. This service
    /// should not be used for any internal communication or any other external
    /// communication. For internal communication, use [`Self::rolegroup_headless_service_name`]
    /// instead.
    pub fn rolegroup_metrics_service_name(&self) -> String {
        format!("{name}-metrics", name = self.object_name())
    }
}

impl<K: Resource> Display for RoleGroupRef<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "role group {}/{} of {}",
            self.role, self.role_group, self.cluster
        ))
    }
}

/// Returns [`Some<u32>`] in case the number of replicas is hard-coded to a certain value.
///
/// This is the case when all `replicas` are set to [`Some<u16>`], in which case they are simply
/// summed.
///
/// The argument `zero_replicas_counting` is a safety mechanism, which allows the caller to decide
/// if an explicit replica count of `0` should be treated as [`None`]. It also means that [`None`]
/// is returned in case no roleGroups are configured at all.
//
// Note: We are using a [`IntoIterator`] combined with `.peekable()` over [`ExactSizeIterator`] to
// have minimal bound requirements on the caller.
pub fn fixed_replica_count<I: IntoIterator<Item = Option<u16>>>(
    replicas: I,
    zero_replicas_counting: ZeroReplicasCounting,
) -> Option<u32> {
    let mut replicas = replicas.into_iter().peekable();

    // An empty role has no fixed replica count when zeros are treated as None.
    if zero_replicas_counting == ZeroReplicasCounting::TreatAsNone && replicas.peek().is_none() {
        return None;
    }

    replicas
        .map(|replicas| match replicas {
            None => None,
            Some(0) if zero_replicas_counting == ZeroReplicasCounting::TreatAsNone => None,
            // The individual replicas are [`u16`]s, so a [`u32`] sum has plenty of space.
            Some(replicas) => Some(u32::from(replicas)),
        })
        .sum()
}

/// Returns the estimated total number of replicas across all role groups.
///
/// Unlike [`fixed_replica_count`], this always returns a value: a role group with an unset (i.e.
/// [`None`]) replica count is assumed to run a single replica. Use this when a best-effort estimate
/// is needed even though the exact number of replicas is not hard-coded.
//
// Note: We are using a [`IntoIterator`] combined with `.peekable()` over [`ExactSizeIterator`] to
// have minimal bound requirements on the caller.
pub fn estimated_replica_count<I: IntoIterator<Item = Option<u16>>>(replicas: I) -> u32 {
    replicas
        .into_iter()
        .map(|replicas| u32::from(replicas.unwrap_or(1)))
        .sum()
}

/// How explicit zero (`0`) replicas on a role group should be counted
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ZeroReplicasCounting {
    /// Treat them as what they are: `Some(0)`.
    TreatAsZero,
    /// Treat them as if the user configured [`None`].
    TreatAsNone,
}
