use crate::status::{
    ClusterCondition, ClusterConditionSet, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder,
};

use k8s_openapi::api::apps::v1::DaemonSet;
use kube::ResourceExt;
use std::cmp;

#[derive(Default)]
pub struct DaemonSetConditionBuilder {
    daemon_sets: Vec<DaemonSet>,
}

impl DaemonSetConditionBuilder {
    pub fn add(&mut self, ds: DaemonSet) {
        self.daemon_sets.push(ds);
    }

    fn available(&self) -> ClusterCondition {
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

impl ConditionBuilder for DaemonSetConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet {
        vec![self.available()].into()
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
