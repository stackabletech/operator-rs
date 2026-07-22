use std::str::FromStr;

use crate::{
    kube::Resource,
    kvp::{Labels, ObjectLabels},
    v2::{
        HasName, NameIsValidLabelValue,
        controller_utils::ContextNames,
        types::operator::{
            ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName, RoleName,
        },
    },
};

// Placeholder label values for resources that a label dimension does not apply to: cluster-shared
// resources (e.g. the RBAC ServiceAccount/RoleBinding) carry `none` for role and role group,
// role-level resources (e.g. a group Listener) carry `none` for the role group, and resources
// whose labels must not change after deployment (e.g. PVC templates) carry `none` as the
// version.
crate::constant!(pub NONE_ROLE_NAME: RoleName = "none");
crate::constant!(pub NONE_ROLE_GROUP_NAME: RoleGroupName = "none");
crate::constant!(pub UNVERSIONED_PRODUCT_VERSION: ProductVersion = "none");

/// Infallible variant of [`crate::kvp::Labels::recommended`]
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

/// Recommended-label helpers for a validated cluster: implementors provide their controller
/// identity and product version once, and inherit the label constructors that every operator
/// otherwise duplicates. Product-specific conveniences (e.g. taking the product's role enum)
/// stay on the implementor.
pub trait HasRecommendedLabels: Resource + HasName + NameIsValidLabelValue + Sized {
    /// The typed identity of the controller managing this cluster.
    fn context_names(&self) -> &ContextNames;

    /// The product version, as used for the `app.kubernetes.io/version` label.
    fn product_version(&self) -> &ProductVersion;

    /// Recommended labels for a resource of the given role and role group; use the placeholder
    /// values above for dimensions that do not apply to the resource.
    fn recommended_labels_for(
        &self,
        role_name: &RoleName,
        role_group_name: &RoleGroupName,
    ) -> Labels {
        self.context_names().recommended_labels(
            self,
            self.product_version(),
            role_name,
            role_group_name,
        )
    }

    /// Recommended labels with the constant [`static@UNVERSIONED_PRODUCT_VERSION`], for
    /// resources whose labels must not change after deployment (e.g. PVC templates).
    fn unversioned_recommended_labels_for(
        &self,
        role_name: &RoleName,
        role_group_name: &RoleGroupName,
    ) -> Labels {
        self.context_names().recommended_labels(
            self,
            &UNVERSIONED_PRODUCT_VERSION,
            role_name,
            role_group_name,
        )
    }
}

/// Infallible variant of [`crate::kvp::Labels::role_selector`]
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

/// Infallible variant of [`crate::kvp::Labels::role_group_selector`]
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
    use std::{borrow::Cow, collections::BTreeMap, sync::OnceLock};

    use crate::{
        k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
        kube::Resource,
        v2::{
            HasName, NameIsValidLabelValue,
            controller_utils::ContextNames,
            kvp::label::{
                HasRecommendedLabels, recommended_labels, role_group_selector, role_selector,
            },
            types::operator::{
                ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName, RoleName,
            },
        },
    };

    struct Cluster {
        object_meta: ObjectMeta,
    }

    impl Cluster {
        fn new() -> Self {
            Self {
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

    impl HasRecommendedLabels for Cluster {
        fn context_names(&self) -> &ContextNames {
            static CONTEXT_NAMES: OnceLock<ContextNames> = OnceLock::new();
            CONTEXT_NAMES.get_or_init(|| ContextNames {
                product_name: ProductName::from_str_unsafe("my-product"),
                operator_name: OperatorName::from_str_unsafe("product.example.org"),
                controller_name: ControllerName::from_str_unsafe("productcluster"),
            })
        }

        fn product_version(&self) -> &ProductVersion {
            static PRODUCT_VERSION: OnceLock<ProductVersion> = OnceLock::new();
            PRODUCT_VERSION.get_or_init(|| ProductVersion::from_str_unsafe("1.2.3"))
        }
    }

    #[test]
    fn trait_provides_recommended_and_unversioned_labels() {
        let cluster = Cluster::new();
        let role = RoleName::from_str_unsafe("my-role");
        let role_group = RoleGroupName::from_str_unsafe("my-role-group");

        let labels: BTreeMap<String, String> =
            cluster.recommended_labels_for(&role, &role_group).into();
        assert_eq!(
            labels.get("app.kubernetes.io/version").map(String::as_str),
            Some("1.2.3")
        );
        assert_eq!(
            labels
                .get("app.kubernetes.io/managed-by")
                .map(String::as_str),
            Some("product.example.org_productcluster")
        );

        let unversioned: BTreeMap<String, String> = cluster
            .unversioned_recommended_labels_for(&role, &role_group)
            .into();
        assert_eq!(
            unversioned
                .get("app.kubernetes.io/version")
                .map(String::as_str),
            Some("none")
        );
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
