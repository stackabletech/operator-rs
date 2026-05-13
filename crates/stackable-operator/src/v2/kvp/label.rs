use stackable_operator::{
    kube::Resource,
    kvp::{Labels, ObjectLabels},
};

use crate::framework::{
    HasName, NameIsValidLabelValue,
    types::operator::{
        ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName, RoleName,
    },
};

/// Infallible variant of [`stackable_operator::kvp::Labels::recommended`]
pub fn recommended_labels(
    owner: &(impl Resource + HasName + NameIsValidLabelValue),
    product_name: &ProductName,
    product_version: &ProductVersion,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
    role_name: &RoleName,
    role_group_name: &RoleGroupName,
) -> Labels {
    let object_labels = ObjectLabels {
        owner,
        app_name: &product_name.to_label_value(),
        app_version: &product_version.to_label_value(),
        operator_name: &operator_name.to_label_value(),
        controller_name: &controller_name.to_label_value(),
        role: &role_name.to_label_value(),
        role_group: &role_group_name.to_label_value(),
    };
    Labels::recommended(&object_labels).expect(
        "Labels should be created because the owner has an object name and all given parameters \
        produce valid label values.",
    )
}

/// Infallible variant of [`stackable_operator::kvp::Labels::role_selector`]
pub fn role_selector(
    owner: &(impl Resource + HasName + NameIsValidLabelValue),
    product_name: &ProductName,
    role_name: &RoleName,
) -> Labels {
    Labels::role_selector(
        owner,
        &product_name.to_label_value(),
        &role_name.to_label_value(),
    )
    .expect("Labels should be created because all given parameters produce valid label values")
}

/// Infallible variant of [`stackable_operator::kvp::Labels::role_group_selector`]
pub fn role_group_selector(
    owner: &(impl Resource + HasName + NameIsValidLabelValue),
    product_name: &ProductName,
    role_name: &RoleName,
    role_group_name: &RoleGroupName,
) -> Labels {
    Labels::role_group_selector(
        owner,
        &product_name.to_label_value(),
        &role_name.to_label_value(),
        &role_group_name.to_label_value(),
    )
    .expect("Labels should be created because all given parameters produce valid label values")
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap};

    use stackable_operator::{
        k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, kube::Resource,
    };

    use crate::framework::{
        HasName, NameIsValidLabelValue,
        kvp::label::{recommended_labels, role_group_selector, role_selector},
        types::operator::{
            ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName, RoleName,
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

    impl NameIsValidLabelValue for Cluster {
        fn to_label_value(&self) -> String {
            self.object_meta
                .name
                .clone()
                .expect("should be set in Cluster::new")
        }
    }

    #[test]
    fn test_recommended_labels() {
        let actual_labels = recommended_labels(
            &Cluster::new(),
            &ProductName::from_str_unsafe("my-product"),
            &ProductVersion::from_str_unsafe("1.0.0"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
            &RoleName::from_str_unsafe("my-role"),
            &RoleGroupName::from_str_unsafe("my-role-group"),
        );

        let expected_labels: BTreeMap<String, String> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/managed-by", "my-operator_my-controller"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/role-group", "my-role-group"),
            ("app.kubernetes.io/version", "1.0.0"),
            ("stackable.tech/vendor", "Stackable"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn test_role_selector() {
        let actual_labels = role_selector(
            &Cluster::new(),
            &ProductName::from_str_unsafe("my-product"),
            &RoleName::from_str_unsafe("my-role"),
        );

        let expected_labels: BTreeMap<String, String> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/name", "my-product"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn test_role_group_selector() {
        let actual_labels = role_group_selector(
            &Cluster::new(),
            &ProductName::from_str_unsafe("my-product"),
            &RoleName::from_str_unsafe("my-role"),
            &RoleGroupName::from_str_unsafe("my-role-group"),
        );

        let expected_labels: BTreeMap<String, String> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/role-group", "my-role-group"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }
}
