use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use strum::Display;

/// Condition types of resources an agent manages inside a product.
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum ManagedConditionType {
    /// The resource exists in the product and is usable.
    Ready,

    /// The last reconciliation applied the spec to the resource in the product.
    Synced,
}

/// The desired state of one condition, without the bookkeeping fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedCondition {
    pub type_: ManagedConditionType,

    pub status: bool,

    /// A CamelCase reason like `Created` or `UnsupportedChange`.
    pub reason: String,

    pub message: String,
}

/// Merges the given conditions into the current ones.
///
/// `lastTransitionTime` only changes if the status of a condition changes. Conditions of other
/// types are kept.
pub fn update_conditions(
    current: &[Condition],
    updates: Vec<ManagedCondition>,
    now: &Time,
) -> Vec<Condition> {
    let mut conditions = current.to_vec();

    for update in updates {
        let type_ = update.type_.to_string();
        let status = if update.status { "True" } else { "False" }.to_owned();
        let existing = conditions
            .iter()
            .position(|condition| condition.type_ == type_);

        let last_transition_time = match existing {
            Some(index) if conditions[index].status == status => {
                conditions[index].last_transition_time.clone()
            }
            _ => now.clone(),
        };

        let condition = Condition {
            last_transition_time,
            message: update.message,
            observed_generation: None,
            reason: update.reason,
            status,
            type_,
        };

        match existing {
            Some(index) => conditions[index] = condition,
            None => conditions.push(condition),
        }
    }

    conditions
}

#[cfg(test)]
mod tests {
    use k8s_openapi::jiff::Timestamp;

    use super::*;

    fn time(seconds: i64) -> Time {
        Time(Timestamp::from_second(seconds).expect("the timestamp should be valid"))
    }

    fn ready(status: bool, reason: &str) -> ManagedCondition {
        ManagedCondition {
            type_: ManagedConditionType::Ready,
            status,
            reason: reason.to_owned(),
            message: String::new(),
        }
    }

    #[test]
    fn add_missing_condition() {
        let conditions = update_conditions(&[], vec![ready(true, "Created")], &time(10));

        assert_eq!(1, conditions.len());
        assert_eq!("Ready", conditions[0].type_);
        assert_eq!("True", conditions[0].status);
        assert_eq!("Created", conditions[0].reason);
        assert_eq!(time(10), conditions[0].last_transition_time);
    }

    #[test]
    fn keep_transition_time_if_status_is_unchanged() {
        let current = update_conditions(&[], vec![ready(true, "Created")], &time(10));

        let conditions = update_conditions(&current, vec![ready(true, "Updated")], &time(20));

        assert_eq!("Updated", conditions[0].reason);
        assert_eq!(time(10), conditions[0].last_transition_time);
    }

    #[test]
    fn update_transition_time_if_status_changes() {
        let current = update_conditions(&[], vec![ready(true, "Created")], &time(10));

        let conditions =
            update_conditions(&current, vec![ready(false, "KafkaRejected")], &time(20));

        assert_eq!("False", conditions[0].status);
        assert_eq!(time(20), conditions[0].last_transition_time);
    }

    #[test]
    fn keep_conditions_of_other_types() {
        let other = Condition {
            last_transition_time: time(5),
            message: String::new(),
            observed_generation: None,
            reason: "Other".to_owned(),
            status: "True".to_owned(),
            type_: "Other".to_owned(),
        };

        let conditions = update_conditions(
            std::slice::from_ref(&other),
            vec![ready(true, "Created")],
            &time(10),
        );

        let types: Vec<_> = conditions
            .iter()
            .map(|condition| condition.type_.as_str())
            .collect();
        assert_eq!(vec!["Other", "Ready"], types);
    }
}
