use std::cmp;

use k8s_openapi::api::apps::v1::Deployment;
use kube::ResourceExt;

use crate::status::condition::{
    ClusterCondition, ClusterConditionSet, ClusterConditionStatus, ClusterConditionType,
    ConditionBuilder,
};

/// Default implementation to build [`ClusterCondition`]s for
/// `Deployment` resources.
///
/// Currently only the `ClusterConditionType::Available` is implemented. This will be extended
/// to support all `ClusterConditionType`s in the future.
#[derive(Default)]
pub struct DeploymentConditionBuilder {
    deployments: Vec<Deployment>,
}

impl ConditionBuilder for DeploymentConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet {
        vec![self.available()].into()
    }
}

impl DeploymentConditionBuilder {
    pub fn add(&mut self, dplmt: Deployment) {
        self.deployments.push(dplmt);
    }

    fn available(&self) -> ClusterCondition {
        let mut available = ClusterConditionStatus::True;
        let mut unavailable_resources = vec![];
        for deployment in &self.deployments {
            let current_status = Self::deployment_available(deployment);

            if current_status != ClusterConditionStatus::True {
                unavailable_resources.push(deployment.name_any())
            }

            available = cmp::max(available, current_status);
        }

        // We need to sort here to make sure roles and role groups are not changing position
        // due to the HashMap (random order) logic.
        unavailable_resources.sort();

        let message = match available {
            ClusterConditionStatus::True => {
                "All Deployments have the requested amount of ready replicas.".to_string()
            }
            ClusterConditionStatus::False => {
                format!("Deployment {unavailable_resources:?} missing ready replicas.")
            }
            ClusterConditionStatus::Unknown => {
                "Deployment status cannot be determined.".to_string()
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
    fn deployment_available(dplmt: &Deployment) -> ClusterConditionStatus {
        let requested_replicas = dplmt
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or_default();
        let available_replicas = dplmt
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
mod tests {
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStatus};

    use crate::status::condition::{
        deployment::DeploymentConditionBuilder, ClusterCondition, ClusterConditionStatus,
        ClusterConditionType, ConditionBuilder,
    };

    fn build_deployment(spec_replicas: i32, available_replicas: i32) -> Deployment {
        Deployment {
            spec: Some(DeploymentSpec {
                replicas: Some(spec_replicas),
                ..DeploymentSpec::default()
            }),
            status: Some(DeploymentStatus {
                available_replicas: Some(available_replicas),
                ..DeploymentStatus::default()
            }),
            ..Deployment::default()
        }
    }

    #[test]
    fn available() {
        let dplmt = build_deployment(3, 3);

        assert_eq!(
            DeploymentConditionBuilder::deployment_available(&dplmt),
            ClusterConditionStatus::True
        );
    }

    #[test]
    fn unavailable() {
        let dplmt = build_deployment(3, 2);

        assert_eq!(
            DeploymentConditionBuilder::deployment_available(&dplmt),
            ClusterConditionStatus::False
        );

        let dplmt = build_deployment(3, 4);

        assert_eq!(
            DeploymentConditionBuilder::deployment_available(&dplmt),
            ClusterConditionStatus::False
        );
    }

    #[test]
    fn condition_available() {
        let mut dplmt_condition_builder = DeploymentConditionBuilder::default();
        dplmt_condition_builder.add(build_deployment(3, 3));

        let conditions = dplmt_condition_builder.build_conditions();

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
    fn condition_unavailable() {
        let mut dplmt_condition_builder = DeploymentConditionBuilder::default();
        dplmt_condition_builder.add(build_deployment(3, 2));

        let conditions = dplmt_condition_builder.build_conditions();

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
