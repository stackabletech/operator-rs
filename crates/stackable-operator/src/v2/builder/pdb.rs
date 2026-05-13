use stackable_operator::{
    builder::pdb::PodDisruptionBudgetBuilder,
    k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector,
    kube::{Resource, api::ObjectMeta},
};

use crate::framework::{
    HasName, HasUid, NameIsValidLabelValue,
    types::operator::{ControllerName, OperatorName, ProductName, RoleName},
};

/// Infallible variant of
/// [`stackable_operator::builder::pdb::PodDisruptionBudgetBuilder::new_with_role`]
pub fn pod_disruption_budget_builder_with_role(
    owner: &(impl Resource<DynamicType = ()> + HasName + NameIsValidLabelValue + HasUid),
    product_name: &ProductName,
    role_name: &RoleName,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
) -> PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, ()> {
    PodDisruptionBudgetBuilder::new_with_role(
        owner,
        &product_name.to_label_value(),
        &role_name.to_label_value(),
        &operator_name.to_label_value(),
        &controller_name.to_label_value(),
    )
    .expect(
        "PodDisruptionBudgetBuilder should be created because the owner has an object name and UID \
        and all given parameters produce valid label values.",
    )
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use stackable_operator::{
        k8s_openapi::{
            api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec},
            apimachinery::pkg::{
                apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference},
                util::intstr::IntOrString,
            },
        },
        kube::Resource,
    };

    use crate::framework::{
        HasName, HasUid, NameIsValidLabelValue,
        builder::pdb::pod_disruption_budget_builder_with_role,
        types::{
            kubernetes::Uid,
            operator::{ControllerName, OperatorName, ProductName, RoleName},
        },
    };

    struct Cluster {
        object_meta: ObjectMeta,
    }

    impl Cluster {
        fn new() -> Self {
            Cluster {
                object_meta: ObjectMeta {
                    name: Some("cluster-name".to_owned()),
                    uid: Some("a6b89911-d48e-4328-88d6-b9251226583d".to_owned()),
                    ..ObjectMeta::default()
                },
            }
        }
    }

    impl Resource for Cluster {
        type DynamicType = ();
        type Scope = ();

        fn kind(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("kind")
        }

        fn group(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("group")
        }

        fn version(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("version")
        }

        fn plural(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("plural")
        }

        fn meta(&self) -> &ObjectMeta {
            &self.object_meta
        }

        fn meta_mut(&mut self) -> &mut ObjectMeta {
            &mut self.object_meta
        }
    }

    impl HasName for Cluster {
        fn to_name(&self) -> String {
            self.object_meta
                .name
                .clone()
                .expect("should be set in Cluster::new")
        }
    }

    impl HasUid for Cluster {
        fn to_uid(&self) -> Uid {
            Uid::from_str_unsafe(
                &self
                    .object_meta
                    .uid
                    .clone()
                    .expect("should be set in Cluster::new"),
            )
        }
    }

    impl NameIsValidLabelValue for Cluster {
        fn to_label_value(&self) -> String {
            self.object_meta
                .name
                .clone()
                .expect("should be set in Cluster::new")
        }
    }

    #[test]
    fn test_pod_disruption_budget_builder_with_role() {
        let actual_pdb = pod_disruption_budget_builder_with_role(
            &Cluster::new(),
            &ProductName::from_str_unsafe("my-product"),
            &RoleName::from_str_unsafe("my-role"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
        )
        .with_max_unavailable(2)
        .build();

        let expected_pdb = PodDisruptionBudget {
            metadata: ObjectMeta {
                labels: Some(
                    [
                        ("app.kubernetes.io/component", "my-role"),
                        ("app.kubernetes.io/instance", "cluster-name"),
                        ("app.kubernetes.io/managed-by", "my-operator_my-controller"),
                        ("app.kubernetes.io/name", "my-product"),
                    ]
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .into(),
                ),
                name: Some("cluster-name-my-role".to_owned()),
                owner_references: Some(vec![OwnerReference {
                    api_version: "group/version".to_owned(),
                    controller: Some(true),
                    kind: "kind".to_owned(),
                    name: "cluster-name".to_owned(),
                    uid: "a6b89911-d48e-4328-88d6-b9251226583d".to_owned(),
                    ..OwnerReference::default()
                }]),
                ..ObjectMeta::default()
            },
            spec: Some(PodDisruptionBudgetSpec {
                max_unavailable: Some(IntOrString::Int(2)),
                selector: Some(LabelSelector {
                    match_labels: Some(
                        [
                            ("app.kubernetes.io/component", "my-role"),
                            ("app.kubernetes.io/instance", "cluster-name"),
                            ("app.kubernetes.io/name", "my-product"),
                        ]
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                        .into(),
                    ),
                    ..LabelSelector::default()
                }),
                ..PodDisruptionBudgetSpec::default()
            }),
            ..PodDisruptionBudget::default()
        };

        assert_eq!(expected_pdb, actual_pdb);
    }
}
