use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::versioned::versioned;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// What happens to a resource in the product when the object managing it is deleted.
    #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    pub enum DeletionPolicy {
        /// Keep the resource in the product.
        #[default]
        Retain,

        /// Delete the resource in the product.
        Delete,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::v1alpha1::DeletionPolicy;

    #[rstest]
    #[case::retain(r#""Retain""#, DeletionPolicy::Retain)]
    #[case::delete(r#""Delete""#, DeletionPolicy::Delete)]
    fn deserialize_policy(#[case] input: &str, #[case] expected: DeletionPolicy) {
        let policy: DeletionPolicy =
            serde_json::from_str(input).expect("the policy should be valid");

        assert_eq!(expected, policy);
    }

    #[test]
    fn retain_by_default() {
        assert_eq!(DeletionPolicy::Retain, DeletionPolicy::default());
    }

    #[test]
    fn reject_unknown_policy() {
        assert!(serde_json::from_str::<DeletionPolicy>(r#""Orphan""#).is_err());
    }
}
