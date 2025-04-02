use std::cmp;

use k8s_openapi::api::apps::v1::DaemonSet;
use kube::ResourceExt;

use crate::status::condition::{
    ClusterCondition, ClusterConditionSet, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder,
};

/// Default implementation to build [`ClusterCondition`]s for
/// `DaemonSet` resources.
///
/// Currently only the `ClusterConditionType::Available` is implemented. This will be extended
/// to support all `ClusterConditionType`s in the future.
#[derive(Default)]
pub struct DaemonSetConditionBuilder {
    daemon_sets: Vec<DaemonSet>,
}

impl ConditionBuilder for DaemonSetConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet {
        vec![self.available()].into()
    }
}

impl DaemonSetConditionBuilder {
    pub fn add(&mut self, ds: DaemonSet) {
        self.daemon_sets.push(ds);
    }

    fn available(&self) -> ClusterCondition {
        let mut available = ClusterConditionStatus::True;
        let mut unavailable_resources = vec![];

        for ds in &self.daemon_sets {
            let current_status = Self::daemon_set_available(ds);

            if current_status != ClusterConditionStatus::True {
                unavailable_resources.push(ds.name_any())
            }

            available = cmp::max(available, current_status);
        }

        // We need to sort here to make sure roles and role groups are not changing position
        // due to the HashMap (random order) logic.
        unavailable_resources.sort();

        let message = match available {
            ClusterConditionStatus::True => {
                "All DaemonSet have the requested amount of ready replicas.".to_string()
            }
            ClusterConditionStatus::False => {
                format!("DaemonSet {unavailable_resources:?} missing ready replicas.")
            }
            ClusterConditionStatus::Unknown => "DaemonSet status cannot be determined.".to_string(),
        };

        ClusterCondition {
            reason: None,
            message: Some(message),
            status: available,
            type_: ClusterConditionType::Available,
            last_transition_time: None,
            last_update_time: None,
        }
    }

    /// Returns a condition "Available: True" if the number of ready Pods is greater than zero.
    /// This is an heuristic that doesn't take into consideration if *all* eligible nodes have
    /// running Pods.
    /// Other fields of the daemon set status have been considered and discarded for being even less
    /// reliable/informative.
    fn daemon_set_available(ds: &DaemonSet) -> ClusterConditionStatus {
        let number_ready = ds
            .status
            .as_ref()
            .map(|status| status.number_ready)
            .unwrap_or_default();

        if number_ready > 0 {
            ClusterConditionStatus::True
        } else {
            ClusterConditionStatus::False
        }
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::apps::v1::{DaemonSet, DaemonSetStatus};

    use crate::status::condition::{
        ClusterCondition, ClusterConditionStatus, ClusterConditionType, ConditionBuilder,
        daemonset::DaemonSetConditionBuilder,
    };

    fn build_ds(number_ready: i32) -> DaemonSet {
        DaemonSet {
            status: Some(DaemonSetStatus {
                number_ready,
                ..DaemonSetStatus::default()
            }),
            ..DaemonSet::default()
        }
    }

    #[test]
    fn daemon_set_available() {
        let ds = build_ds(1);

        assert_eq!(
            DaemonSetConditionBuilder::daemon_set_available(&ds),
            ClusterConditionStatus::True
        );
    }

    #[test]
    fn daemon_set_unavailable() {
        let ds = build_ds(0);
        assert_eq!(
            DaemonSetConditionBuilder::daemon_set_available(&ds),
            ClusterConditionStatus::False
        );
    }

    #[test]
    fn daemon_set_condition_available() {
        let mut ds_condition_builder = DaemonSetConditionBuilder::default();
        ds_condition_builder.add(build_ds(1));

        let conditions = ds_condition_builder.build_conditions();

        let got = conditions
            .conditions
            .get::<usize>(ClusterConditionType::Available.into())
            .cloned()
            .unwrap()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::True,
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
    }

    #[test]
    fn daemon_set_condition_unavailable() {
        let mut ds_condition_builder = DaemonSetConditionBuilder::default();
        ds_condition_builder.add(build_ds(0));

        let conditions = ds_condition_builder.build_conditions();

        let got = conditions
            .conditions
            .get::<usize>(ClusterConditionType::Available.into())
            .cloned()
            .unwrap()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::False,
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
    }
}
