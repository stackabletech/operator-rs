use crate::status::{
    update_condition, ClusterCondition, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder, HasStatusCondition,
};

use k8s_openapi::api::apps::v1::DaemonSet;
use kube::ResourceExt;
use std::cmp;

pub struct DaemonSetConditionBuilder<'a, T: HasStatusCondition> {
    resource: &'a T,
    daemon_sets: Vec<DaemonSet>,
}

impl<'a, T: HasStatusCondition> DaemonSetConditionBuilder<'a, T> {
    pub fn new(resource: &'a T) -> DaemonSetConditionBuilder<T> {
        DaemonSetConditionBuilder {
            resource,
            daemon_sets: Vec::new(),
        }
    }

    pub fn add(&mut self, ds: DaemonSet) {
        self.daemon_sets.push(ds);
    }

    pub fn available(&self) -> ClusterCondition {
        let opt_old_available = self
            .resource
            .conditions()
            .iter()
            .find(|cond| cond.type_ == ClusterConditionType::Available)
            .cloned();

        let mut available = ClusterConditionStatus::True;
        let mut unavailable_ds = vec![];

        for ds in &self.daemon_sets {
            let current_status = daemon_set_available(ds);

            if current_status != ClusterConditionStatus::True {
                unavailable_ds.push(ds.name_any())
            }

            available = cmp::max(available, current_status);
        }

        let message = match available {
            ClusterConditionStatus::True => {
                "All DaemonSet have the requested amount of ready replicas.".to_string()
            }
            ClusterConditionStatus::False => {
                format!("DaemonSet {unavailable_ds:?} missing ready replicas.")
            }
            ClusterConditionStatus::Unknown => "DaemonSet status cannot be determined.".to_string(),
        };

        update_condition(
            ClusterConditionType::Available,
            opt_old_available,
            available,
            &message,
        )
    }
}

impl<'a, T: HasStatusCondition> ConditionBuilder for DaemonSetConditionBuilder<'a, T> {
    fn build_conditions(&self) -> Vec<ClusterCondition> {
        vec![self.available()]
    }
}

fn daemon_set_available(ds: &DaemonSet) -> ClusterConditionStatus {
    let number_unavailable = ds
        .status
        .as_ref()
        .and_then(|status| status.number_unavailable)
        .unwrap_or_default();

    if number_unavailable == 0 {
        ClusterConditionStatus::True
    } else {
        ClusterConditionStatus::False
    }
}
