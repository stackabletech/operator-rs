pub mod daemonset;
pub mod operations;
pub mod statefulset;

use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use strum::EnumCount;

/// A **data structure** that contains a vector of `ClusterCondition`s.
/// Should usually be implemented on the status of a `CustomResource` or the `CustomResource` itself.
pub trait HasStatusCondition {
    fn conditions(&self) -> Vec<ClusterCondition>;
}

/// A **data structure** that produces a `ClusterConditionSet` containing all required
/// `ClusterCondition`s.
pub trait ConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet;
}

/// Computes the final conditions to be set in the operator status condition field.
///
/// # Arguments
///
/// * `resource` - A cluster resource or status implementing [`HasStatusCondition`] in order to
///    retrieve the "current" conditions set in the cluster. This is required to  compute
///    condition change and set proper update / transition times.
/// * `condition_builders` - A slice of structs implementing [`ConditionBuilder`]. This can be a
///    one of the predefined ConditionBuilders like `DaemonSetConditionBuilder` or a custom
///    implementation for special resources or different behavior.
///
/// # Examples
/// ```
/// use stackable_operator::status::condition::daemonset::DaemonSetConditionBuilder;
/// use stackable_operator::status::condition::statefulset::StatefulSetConditionBuilder;
/// use k8s_openapi::api::apps::v1::{DaemonSet, StatefulSet};
/// use stackable_operator::status::condition::{ClusterCondition, ConditionBuilder, HasStatusCondition, compute_conditions};
///
/// struct ClusterStatus {
///     conditions: Vec<ClusterCondition>
/// }
///
/// impl HasStatusCondition for ClusterStatus {
///     fn conditions(&self) -> Vec<ClusterCondition> {
///         self.conditions.clone()
///     }
/// }
///
/// let mut daemonset_condition_builder = DaemonSetConditionBuilder::default();
/// daemonset_condition_builder.add(DaemonSet::default());
///
/// let mut statefulset_condition_builder = StatefulSetConditionBuilder::default();
/// statefulset_condition_builder.add(StatefulSet::default());
///
/// let old_status = ClusterStatus {
///     conditions: vec![]
/// };
///
/// let new_status = ClusterStatus {
///     conditions: compute_conditions(&old_status,
///         &[
///             &daemonset_condition_builder as &dyn ConditionBuilder,
///             &statefulset_condition_builder as &dyn ConditionBuilder
///         ]
///     )
/// };
///
/// ```
pub fn compute_conditions<T: HasStatusCondition>(
    resource: &T,
    condition_builders: &[&dyn ConditionBuilder],
) -> Vec<ClusterCondition> {
    let mut new_resource_conditions = ClusterConditionSet::new();
    for cb in condition_builders {
        let conditions: ClusterConditionSet = cb.build_conditions();
        new_resource_conditions = new_resource_conditions.merge(conditions, update_message);
    }

    let old_resource_conditions: ClusterConditionSet = resource.conditions().into();

    old_resource_conditions
        .merge(new_resource_conditions, update_timestamps)
        .into()
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Last time the condition transitioned from one status to another.
    pub last_transition_time: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The last time this condition was updated.
    pub last_update_time: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A human readable message indicating details about the transition.
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The reason for the condition's last transition.
    pub reason: Option<String>,
    /// Status of the condition, one of True, False, Unknown.
    pub status: ClusterConditionStatus,
    /// Type of deployment condition.
    #[serde(rename = "type")]
    pub type_: ClusterConditionType,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    EnumCount,
    Eq,
    Hash,
    JsonSchema,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum ClusterConditionType {
    #[default]
    /// Available indicates that the binary maintained by the operator (eg: zookeeper for the
    /// zookeeper-operator), is functional and available in the cluster.
    Available,
    /// Degraded indicates that the operand is not functioning completely. An example of a degraded
    /// state would be if there should be 5 copies of the operand running but only 4 are running.
    /// It may still be available, but it is degraded.
    Degraded,
    /// Progressing indicates that the operator is actively making changes to the binary maintained
    /// by the operator (eg: zookeeper for the zookeeper-operator).
    Progressing,
    /// Paused indicates that the operator is not reconciling the cluster. This may be used for
    /// debugging or operator updating.
    ReconciliationPaused,
    /// Stopped indicates that all the cluster replicas are scaled down to 0. All resources (e.g.
    /// ConfigMaps, Services etc.) are kept.
    Stopped,
}

#[derive(
    Clone, Debug, Default, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum ClusterConditionStatus {
    #[default]
    /// True means a resource is in the condition.
    True,
    /// False means a resource is not in the condition.
    False,
    /// Unknown means kubernetes cannot decide if a resource is in the condition or not.
    Unknown,
}

#[derive(Clone, Default)]
/// Helper struct to order and merge `ClusterCondition` objects.
pub struct ClusterConditionSet {
    conditions: Vec<Option<ClusterCondition>>,
}

impl ClusterConditionSet {
    pub fn new() -> Self {
        ClusterConditionSet {
            // We use this as a quasi "Set" where each ClusterConditionType has its fixed position
            // This ensures ordering, and in contrast to e.g. a
            // BTreeMap<ClusterConditionType, ClusterCondition>, prevents shenanigans like adding a
            // ClusterCondition (as value) with a different ClusterConditionType than its key.
            // See "put".
            conditions: vec![None; ClusterConditionType::COUNT],
        }
    }

    /// Adds a [`ClusterCondition`] to its assigned index in the conditions vector.
    fn put(&mut self, condition: ClusterCondition) {
        let index = condition.type_ as usize;
        self.conditions[index] = Some(condition);
    }

    /// Merges two [`ClusterConditionSet`]s. The condition_combiner implements the strategy used to merge two conditions of
    /// of the same type_.
    fn merge(
        self,
        other: ClusterConditionSet,
        condition_combiner: fn(ClusterCondition, ClusterCondition) -> ClusterCondition,
    ) -> ClusterConditionSet {
        let mut result = ClusterConditionSet::new();

        for (old_condition, new_condition) in self
            .conditions
            .into_iter()
            .zip(other.conditions.into_iter())
        {
            if let Some(condition) = match (old_condition, new_condition) {
                (Some(old), Some(new)) => Some(condition_combiner(old, new)),
                (Some(old), None) => Some(old),
                (None, Some(new)) => Some(new),
                _ => None,
            } {
                result.put(condition);
            };
        }

        result
    }
}

/// A condition combiner strategy where the timestamps are updated to reflect a
/// state transition (if needed).
fn update_timestamps(
    old_condition: ClusterCondition,
    new_condition: ClusterCondition,
) -> ClusterCondition {
    // sanity check
    assert_eq!(old_condition.type_, new_condition.type_);

    let now = Time(Utc::now());
    // No change in status -> update "last_update_time" and keep "last_transition_time"
    if old_condition.status == new_condition.status {
        ClusterCondition {
            last_update_time: Some(now),
            last_transition_time: old_condition.last_transition_time,
            ..new_condition
        }
    // Change in status -> set new "last_update_time" and "last_transition_time"
    } else {
        ClusterCondition {
            last_update_time: Some(now.clone()),
            last_transition_time: Some(now),
            ..new_condition
        }
    }
}

/// A condition combiner strategy with the following properties:
/// 1. It preserves the condition with the highest status.
/// 2. It joins the previous messages to the current one if both conditions
/// have the same status.
fn update_message(
    old_condition: ClusterCondition,
    new_condition: ClusterCondition,
) -> ClusterCondition {
    // sanity check
    assert_eq!(old_condition.type_, new_condition.type_);

    match old_condition.status.cmp(&new_condition.status) {
        std::cmp::Ordering::Equal => {
            let message = Some(
                vec![old_condition.message, new_condition.message]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<String>>()
                    .join("\n"),
            );

            ClusterCondition {
                message,
                ..new_condition
            }
        }
        std::cmp::Ordering::Less => new_condition,
        std::cmp::Ordering::Greater => old_condition,
    }
}

impl From<ClusterConditionSet> for Vec<ClusterCondition> {
    fn from(value: ClusterConditionSet) -> Self {
        value.conditions.into_iter().flatten().collect()
    }
}

impl From<Vec<ClusterCondition>> for ClusterConditionSet {
    fn from(value: Vec<ClusterCondition>) -> Self {
        let mut result = ClusterConditionSet::new();
        for c in value {
            result.put(c);
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct TestClusterCondition {}
    impl HasStatusCondition for TestClusterCondition {
        fn conditions(&self) -> Vec<ClusterCondition> {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::Unknown,
                message: Some("TestClusterCondition=Unknown".into()),
                ..ClusterCondition::default()
            }]
        }
    }

    struct AvailableFalseConditionBuilder1 {}
    impl ConditionBuilder for AvailableFalseConditionBuilder1 {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::False,
                message: Some("AvailableFalseConditionBuilder".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }

    struct AvailableFalseConditionBuilder2 {}
    impl ConditionBuilder for AvailableFalseConditionBuilder2 {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::False,
                message: Some("AvailableFalseConditionBuilder_2".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }

    struct AvailableTrueConditionBuilder1 {}
    impl ConditionBuilder for AvailableTrueConditionBuilder1 {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::True,
                message: Some("AvailableTrueConditionBuilder_1".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }

    struct AvailableTrueConditionBuilder2 {}
    impl ConditionBuilder for AvailableTrueConditionBuilder2 {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::True,
                message: Some("AvailableTrueConditionBuilder_2".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }

    struct AvailableUnknownConditionBuilder {}
    impl ConditionBuilder for AvailableUnknownConditionBuilder {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: ClusterConditionType::Available,
                status: ClusterConditionStatus::Unknown,
                message: Some("AvailableUnknownConditionBuilder".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }

    #[test]
    pub fn test_compute_conditions_with_transition() {
        let resource = TestClusterCondition {};
        let condition_builders = &[&AvailableTrueConditionBuilder1 {} as &dyn ConditionBuilder];

        let got = compute_conditions(&resource, condition_builders)
            .get(0)
            .cloned()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::True,
            message: Some("AvailableTrueConditionBuilder_1".into()),
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
        assert_eq!(got.message, expected.message);
        assert_eq!(got.last_transition_time, got.last_update_time);
        assert!(got.last_transition_time.is_some());
    }

    #[test]
    pub fn test_compute_conditions_message_concatenation() {
        let resource = TestClusterCondition {};
        let condition_builders = &[
            &AvailableTrueConditionBuilder1 {} as &dyn ConditionBuilder,
            &AvailableTrueConditionBuilder2 {} as &dyn ConditionBuilder,
        ];

        let got = compute_conditions(&resource, condition_builders)
            .get(0)
            .cloned()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::True,
            message: Some(
                "AvailableTrueConditionBuilder_1\nAvailableTrueConditionBuilder_2".into(),
            ),
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
        assert_eq!(got.message, expected.message);
    }

    #[test]
    pub fn test_compute_conditions_message_concatenation_with_different_status() {
        let resource = TestClusterCondition {};
        let condition_builders = &[
            &AvailableFalseConditionBuilder1 {} as &dyn ConditionBuilder,
            &AvailableTrueConditionBuilder1 {} as &dyn ConditionBuilder,
            &AvailableFalseConditionBuilder2 {} as &dyn ConditionBuilder,
            &AvailableTrueConditionBuilder2 {} as &dyn ConditionBuilder,
        ];

        let got = compute_conditions(&resource, condition_builders)
            .get(0)
            .cloned()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::False,
            message: Some(
                "AvailableFalseConditionBuilder\nAvailableFalseConditionBuilder_2".into(),
            ),
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
        assert_eq!(got.message, expected.message);
    }

    #[test]
    pub fn test_compute_conditions_status_priority() {
        let resource = TestClusterCondition {};
        let condition_builders = &[
            &AvailableUnknownConditionBuilder {} as &dyn ConditionBuilder,
            &AvailableFalseConditionBuilder1 {} as &dyn ConditionBuilder,
            &AvailableTrueConditionBuilder1 {} as &dyn ConditionBuilder,
        ];

        let got = compute_conditions(&resource, condition_builders)
            .get(0)
            .cloned()
            .unwrap();

        let expected = ClusterCondition {
            type_: ClusterConditionType::Available,
            status: ClusterConditionStatus::Unknown,
            message: Some("AvailableUnknownConditionBuilder".into()),
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
        assert_eq!(got.message, expected.message);
    }
}
