use crate::status::condition::{
    ClusterCondition, ClusterConditionSet, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder,
};

use k8s_openapi::api::apps::v1::StatefulSet;
use kube::ResourceExt;
use std::cmp;

/// Default implementation to build [`crate::status::condition::ClusterCondition`]s for
/// `StatefulSet` resources.
///
/// Currently only the `ClusterConditionType::Available` is implemented. This will be extended
/// to support all `ClusterConditionType`s in the future.
#[derive(Default)]
pub struct StatefulSetConditionBuilder {
    stateful_sets: Vec<StatefulSet>,
}

impl ConditionBuilder for StatefulSetConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet {
        vec![self.available()].into()
    }
}

impl StatefulSetConditionBuilder {
    pub fn add(&mut self, sts: StatefulSet) {
        self.stateful_sets.push(sts);
    }

    fn available(&self) -> ClusterCondition {
        let mut available = ClusterConditionStatus::True;
        let mut unavailable_resources = vec![];
        for sts in &self.stateful_sets {
            let current_status = Self::stateful_set_available(sts);

            if current_status != ClusterConditionStatus::True {
                unavailable_resources.push(sts.name_any())
            }

            available = cmp::max(available, current_status);
        }

        // We need to sort here to make sure roles and role groups are not changing position
        // due to the HashMap (random order) logic.
        unavailable_resources.sort();

        let message = match available {
            ClusterConditionStatus::True => {
                "All StatefulSet have the requested amount of ready replicas.".to_string()
            }
            ClusterConditionStatus::False => {
                format!("StatefulSet {unavailable_resources:?} missing ready replicas.")
            }
            ClusterConditionStatus::Unknown => {
                "StatefulSet status cannot be determined.".to_string()
            }
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

    /// Returns a condition "Available: True" if the number of requested replicas matches
    /// the number of available replicas. In addition, there needs to be at least one replica
    /// available.
    fn stateful_set_available(sts: &StatefulSet) -> ClusterConditionStatus {
        let requested_replicas = sts
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or_default();
        let available_replicas = sts
            .status
            .as_ref()
            .and_then(|status| status.available_replicas)
            .unwrap_or_default();

        if requested_replicas == available_replicas && requested_replicas != 0 {
            ClusterConditionStatus::True
        } else {
            ClusterConditionStatus::False
        }
    }
}

#[cfg(test)]
mod test {
    use crate::status::condition::statefulset::StatefulSetConditionBuilder;
    use crate::status::condition::{
        ClusterCondition, ClusterConditionStatus, ClusterConditionType, ConditionBuilder,
    };
    use k8s_openapi::api::apps::v1::{StatefulSet, StatefulSetSpec, StatefulSetStatus};

    fn build_sts(spec_replicas: i32, available_replicas: i32) -> StatefulSet {
        StatefulSet {
            spec: Some(StatefulSetSpec {
                replicas: Some(spec_replicas),
                ..StatefulSetSpec::default()
            }),
            status: Some(StatefulSetStatus {
                available_replicas: Some(available_replicas),
                ..StatefulSetStatus::default()
            }),
            ..StatefulSet::default()
        }
    }

    #[test]
    fn test_stateful_set_available_true() {
        let sts = build_sts(3, 3);

        assert_eq!(
            StatefulSetConditionBuilder::stateful_set_available(&sts),
            ClusterConditionStatus::True
        );
    }

    #[test]
    fn test_stateful_set_available_false() {
        let sts = build_sts(3, 2);

        assert_eq!(
            StatefulSetConditionBuilder::stateful_set_available(&sts),
            ClusterConditionStatus::False
        );

        let sts = build_sts(3, 4);

        assert_eq!(
            StatefulSetConditionBuilder::stateful_set_available(&sts),
            ClusterConditionStatus::False
        );
    }

    #[test]
    fn test_stateful_set_available_condition_true() {
        let mut sts_condition_builder = StatefulSetConditionBuilder::default();
        sts_condition_builder.add(build_sts(3, 3));

        let conditions = sts_condition_builder.build_conditions();

        let got = conditions
            .conditions
            .get(ClusterConditionType::Available as usize)
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
    fn test_stateful_set_available_condition_false() {
        let mut sts_condition_builder = StatefulSetConditionBuilder::default();
        sts_condition_builder.add(build_sts(3, 2));

        let conditions = sts_condition_builder.build_conditions();

        let got = conditions
            .conditions
            .get(ClusterConditionType::Available as usize)
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
