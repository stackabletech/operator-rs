use crate::{
    commons::cluster_operation::ClusterOperation,
    status::condition::{
        ClusterCondition, ClusterConditionSet, ClusterConditionStatus, ClusterConditionType,
        ConditionBuilder,
    },
};

/// Default implementation to build [`ClusterCondition`]s for
/// the ClusterOperation.
#[derive(Debug, Clone)]
pub struct ClusterOperationsConditionBuilder<'a> {
    cluster_operation: &'a ClusterOperation,
}

impl<'a> ConditionBuilder for ClusterOperationsConditionBuilder<'a> {
    fn build_conditions(&self) -> ClusterConditionSet {
        vec![self.reconciliation_paused(), self.cluster_stopped()].into()
    }
}

impl<'a> ClusterOperationsConditionBuilder<'a> {
    pub const fn new(cluster_operation: &'a ClusterOperation) -> Self {
        Self { cluster_operation }
    }

    /// Returns the `ReconciliationPaused` cluster condition.
    fn reconciliation_paused(&self) -> ClusterCondition {
        let status = if self.cluster_operation.reconciliation_paused {
            ClusterConditionStatus::True
        } else {
            ClusterConditionStatus::False
        };

        let message = match status {
            ClusterConditionStatus::True => {
                "The cluster reconciliation is paused. Only the cluster status is reconciled."
                    .to_string()
            }
            ClusterConditionStatus::False => "The cluster is reconciled normally.".to_string(),
            ClusterConditionStatus::Unknown => {
                "The cluster reconciliation status could not be determined.".to_string()
            }
        };

        ClusterCondition {
            reason: None,
            message: Some(message),
            status,
            type_: ClusterConditionType::ReconciliationPaused,
            last_transition_time: None,
            last_update_time: None,
        }
    }

    /// Returns the `Stopped` cluster condition.
    fn cluster_stopped(&self) -> ClusterCondition {
        let status =
            if self.cluster_operation.stopped && self.cluster_operation.reconciliation_paused {
                ClusterConditionStatus::Unknown
            } else if self.cluster_operation.stopped {
                ClusterConditionStatus::True
            } else {
                ClusterConditionStatus::False
            };

        let message = match status {
            ClusterConditionStatus::True => {
                "The cluster is stopped.".to_string()
            }
            ClusterConditionStatus::False => {
                "The cluster is running.".to_string()
            }
            ClusterConditionStatus::Unknown => {
                "The cluster stopped status could not be determined. This might be due to the cluster reconciliation being paused.".to_string()
            }
        };

        ClusterCondition {
            reason: None,
            message: Some(message),
            status,
            type_: ClusterConditionType::Stopped,
            last_transition_time: None,
            last_update_time: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;

    #[rstest]
    #[case::not_paused_not_stopped(
        false,
        false,
        ClusterConditionStatus::False,
        ClusterConditionStatus::False
    )]
    #[case::paused_not_stopped(
        true,
        false,
        ClusterConditionStatus::True,
        ClusterConditionStatus::False
    )]
    #[case::not_paused_stopped(
        false,
        true,
        ClusterConditionStatus::False,
        ClusterConditionStatus::True
    )]
    #[case::paused_stopped(
        true,
        true,
        ClusterConditionStatus::True,
        ClusterConditionStatus::Unknown
    )]
    fn cluster_operation_condition(
        #[case] reconciliation_paused: bool,
        #[case] stopped: bool,
        #[case] expected_paused_status: ClusterConditionStatus,
        #[case] expected_stopped_status: ClusterConditionStatus,
    ) {
        let cluster_operation = ClusterOperation {
            reconciliation_paused,
            stopped,
        };

        let op_condition_builder = ClusterOperationsConditionBuilder::new(&cluster_operation);
        let conditions = op_condition_builder.build_conditions();

        let got = conditions
            .conditions
            .get::<usize>(ClusterConditionType::ReconciliationPaused.into())
            .cloned()
            .unwrap()
            .unwrap();

        assert_eq!(got.status, expected_paused_status);

        let got = conditions
            .conditions
            .get::<usize>(ClusterConditionType::Stopped.into())
            .cloned()
            .unwrap()
            .unwrap();

        assert_eq!(got.status, expected_stopped_status);
    }
}
