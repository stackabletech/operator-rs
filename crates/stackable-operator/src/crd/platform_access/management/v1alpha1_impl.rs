use crate::crd::platform_access::management::v1alpha1::{
    AdoptionPolicy, DeletionPolicy, ManagementPolicy,
};

impl ManagementPolicy {
    /// Whether the agent takes over a resource that already exists in the product.
    pub fn adopts_existing(&self) -> bool {
        self.adoption == AdoptionPolicy::Adopt
    }

    /// Whether the agent deletes the resource in the product when this object is deleted.
    pub fn deletes_on_cleanup(&self) -> bool {
        self.deletion == DeletionPolicy::Delete
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::defaults("{}", false, false)]
    #[case::adopt(r#"{"adoption": "Adopt"}"#, true, false)]
    #[case::delete(r#"{"deletion": "Delete"}"#, false, true)]
    #[case::explicit_defaults(r#"{"adoption": "Refuse", "deletion": "Retain"}"#, false, false)]
    fn deserialize_policy(
        #[case] input: &str,
        #[case] adopts_existing: bool,
        #[case] deletes_on_cleanup: bool,
    ) {
        let policy: ManagementPolicy =
            serde_json::from_str(input).expect("the policy should be valid");

        assert_eq!(adopts_existing, policy.adopts_existing());
        assert_eq!(deletes_on_cleanup, policy.deletes_on_cleanup());
    }

    #[test]
    fn reject_unknown_policy() {
        assert!(serde_json::from_str::<ManagementPolicy>(r#"{"deletion": "Orphan"}"#).is_err());
    }
}
