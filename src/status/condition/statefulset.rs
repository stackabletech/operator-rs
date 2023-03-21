use crate::status::{
    update_condition, ClusterCondition, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder, HasStatusCondition,
};

use k8s_openapi::api::apps::v1::StatefulSet;
use kube::ResourceExt;
use std::cmp;

pub struct StatefulSetConditionBuilder<'a, T: HasStatusCondition> {
    resource: &'a T,
    stateful_sets: Vec<StatefulSet>,
}

impl<'a, T: HasStatusCondition> StatefulSetConditionBuilder<'a, T> {
    pub fn new(resource: &'a T) -> StatefulSetConditionBuilder<T> {
        StatefulSetConditionBuilder {
            resource,
            stateful_sets: Vec::new(),
        }
    }

    pub fn add(&mut self, sts: StatefulSet) {
        self.stateful_sets.push(sts);
    }

    pub fn available(&self) -> ClusterCondition {
        let opt_old_available = self
            .resource
            .conditions()
            .iter()
            .find(|cond| cond.type_ == ClusterConditionType::Available)
            .cloned();

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

        update_condition(
            ClusterConditionType::Available,
            opt_old_available,
            available,
            &message,
        )
    }
}

impl<'a, T: HasStatusCondition> ConditionBuilder for StatefulSetConditionBuilder<'a, T> {
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
