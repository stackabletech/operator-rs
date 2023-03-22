use crate::status::{
    ClusterCondition, ClusterConditionStatus, ClusterConditionType, ConditionBuilder,
};

use k8s_openapi::api::apps::v1::StatefulSet;
use kube::ResourceExt;
use std::cmp;

#[derive(Default)]
pub struct StatefulSetConditionBuilder {
    stateful_sets: Vec<StatefulSet>,
}

impl StatefulSetConditionBuilder {
    pub fn add(&mut self, sts: StatefulSet) {
        self.stateful_sets.push(sts);
    }

    fn available(&self) -> ClusterCondition {
        let mut available = ClusterConditionStatus::True;
        let mut unavailable_sts = vec![];
        for sts in &self.stateful_sets {
            let current_status = stateful_set_available(sts);

            if current_status != ClusterConditionStatus::True {
                unavailable_sts.push(sts.name_any())
            }

            available = cmp::max(available, current_status);
        }

        let message = match available {
            ClusterConditionStatus::True => {
                "All StatefulSet have the requested amount of ready replicas.".to_string()
            }
            ClusterConditionStatus::False => {
                format!("StatefulSet {unavailable_sts:?} missing ready replicas.")
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
}

impl ConditionBuilder for StatefulSetConditionBuilder {
    fn build_conditions(&self) -> Vec<ClusterCondition> {
        vec![self.available()]
    }
}

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

    if requested_replicas == available_replicas {
        ClusterConditionStatus::True
    } else {
        ClusterConditionStatus::False
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {}
}
