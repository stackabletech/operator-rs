use std::str::FromStr;

use crate::{
    kvp::{Label, Labels, consts::K8S_APP_MANAGED_BY_KEY},
    v2::{
        NameIsValidLabelValue,
        types::operator::{
            ClusterName, ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName,
            RoleName,
        },
    },
};

/// Creates the recommended labels for cluster resources, like ServiceAccounts.
pub fn recommended_labels_for_cluster_resources(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    product_version: &ProductVersion,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_version(product_version),
        label_app_kubernetes_io_managed_by(operator_name, controller_name),
        label_stackable_tech_vendor(),
    ])
}

/// Creates the recommended labels for role resources, like discovery ConfigMaps.
pub fn recommended_labels_for_role_resources(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    product_version: &ProductVersion,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
    role_name: &RoleName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_version(product_version),
        label_app_kubernetes_io_component(role_name),
        label_app_kubernetes_io_managed_by(operator_name, controller_name),
        label_stackable_tech_vendor(),
    ])
}

/// Creates the role selector.
///
/// The returned labels are a subset of the recommended labels for role resources.
pub fn role_selector(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    role_name: &RoleName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_component(role_name),
    ])
}

/// Creates the recommended labels for role group resources, like StatefulSets.
pub fn recommended_labels_for_role_group_resources(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    product_version: &ProductVersion,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
    role_name: &RoleName,
    role_group_name: &RoleGroupName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_version(product_version),
        label_app_kubernetes_io_component(role_name),
        label_app_kubernetes_io_role_group(role_group_name),
        label_app_kubernetes_io_managed_by(operator_name, controller_name),
        label_stackable_tech_vendor(),
    ])
}

/// Creates the recommended labels for role group resources which cannot be mutated and should
/// therefore not include product version, like PersistentVolumeClaims.
pub fn recommended_labels_for_unversioned_role_group_resources(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
    role_name: &RoleName,
    role_group_name: &RoleGroupName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_component(role_name),
        label_app_kubernetes_io_role_group(role_group_name),
        label_app_kubernetes_io_managed_by(operator_name, controller_name),
        label_stackable_tech_vendor(),
    ])
}

/// Creates the role group selector.
///
/// The returned labels are a subset of the recommended labels for role group resources.
pub fn role_group_selector(
    cluster_name: &ClusterName,
    product_name: &ProductName,
    role_name: &RoleName,
    role_group_name: &RoleGroupName,
) -> Labels {
    Labels::from_iter([
        label_app_kubernetes_io_instance(cluster_name),
        label_app_kubernetes_io_name(product_name),
        label_app_kubernetes_io_component(role_name),
        label_app_kubernetes_io_role_group(role_group_name),
    ])
}

/// Creates the `app.kubernetes.io/instance` label with the given cluster name as value.
pub fn label_app_kubernetes_io_instance(cluster_name: &ClusterName) -> Label {
    Label::instance(&cluster_name.to_label_value())
        .expect("the value implements NameIsValidLabelValue and is therefore a valid label value")
}

/// Creates the `app.kubernetes.io/name` label with the given product name as value.
pub fn label_app_kubernetes_io_name(product_name: &ProductName) -> Label {
    Label::name(&product_name.to_label_value())
        .expect("the value implements NameIsValidLabelValue and is therefore a valid label value")
}

/// Creates the `app.kubernetes.io/version` label with the given product version as value.
pub fn label_app_kubernetes_io_version(product_version: &ProductVersion) -> Label {
    Label::version(&product_version.to_label_value())
        .expect("the value implements NameIsValidLabelValue and is therefore a valid label value")
}

/// Creates the `app.kubernetes.io/managed-by` label. Its value is the full controller name built
/// from the given operator and controller name.
pub fn label_app_kubernetes_io_managed_by(
    operator_name: &OperatorName,
    controller_name: &ControllerName,
) -> Label {
    let full_controller_name = full_controller_name(operator_name, controller_name);
    Label::try_from((K8S_APP_MANAGED_BY_KEY, full_controller_name))
        .expect("the statically defined key is valid and the value implements NameIsValidLabelValue, so this is a valid label")
}

/// Creates the `app.kubernetes.io/component` label with the given role as value.
pub fn label_app_kubernetes_io_component(role_name: &RoleName) -> Label {
    Label::component(&role_name.to_label_value())
        .expect("the value implements NameIsValidLabelValue and is therefore a valid label value")
}

/// Creates the `app.kubernetes.io/role-group` label with the given role group as value.
pub fn label_app_kubernetes_io_role_group(role_group_name: &RoleGroupName) -> Label {
    Label::role_group(&role_group_name.to_label_value())
        .expect("the value implements NameIsValidLabelValue and is therefore a valid label value")
}

/// Creates the Stackable-specific vendor label.
pub fn label_stackable_tech_vendor() -> Label {
    Label::stackable_vendor()
}

/// Joins the operator and controller name with an underscore to build the full controller name.
///
/// If the full controller name exceeds the maximum length of a `ControllerName`, only the operator
/// name is returned. This limit is unlikely to be hit in practice: even a long operator name like
/// "zookeeper.stackable.tech" (24 characters) combined with a long controller name like
/// "zookeepercluster" (16 characters) stays well below the 63 character limit of a `ControllerName`.
fn full_controller_name(
    operator_name: &OperatorName,
    controller_name: &ControllerName,
) -> ControllerName {
    let full_controller_name = format!("{operator_name}_{controller_name}");

    ControllerName::from_str(&full_controller_name).unwrap_or_else(|_| {
        ControllerName::from_str(operator_name.as_ref()).expect(
            "the operator name is a valid ControllerName because both types share the same constraints",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn recommended_labels_for_cluster_resources_produces_expected_labels() {
        let actual_labels = recommended_labels_for_cluster_resources(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &ProductVersion::from_str_unsafe("1.0.0"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
        );

        let expected_labels: BTreeMap<_, _> = [
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/managed-by", "my-operator_my-controller"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/version", "1.0.0"),
            ("stackable.tech/vendor", "Stackable"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn recommended_labels_for_role_resources_produces_expected_labels() {
        let actual_labels = recommended_labels_for_role_resources(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &ProductVersion::from_str_unsafe("1.0.0"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
            &RoleName::from_str_unsafe("my-role"),
        );

        let expected_labels: BTreeMap<_, _> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/managed-by", "my-operator_my-controller"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/version", "1.0.0"),
            ("stackable.tech/vendor", "Stackable"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn role_selector_produces_expected_labels() {
        let actual_labels = role_selector(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &RoleName::from_str_unsafe("my-role"),
        );

        let expected_labels: BTreeMap<_, _> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/name", "my-product"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn role_selector_is_subset_of_recommended_role_labels() {
        let cluster_name = ClusterName::from_str_unsafe("cluster-name");
        let product_name = ProductName::from_str_unsafe("my-product");
        let product_version = ProductVersion::from_str_unsafe("1.0.0");
        let operator_name = OperatorName::from_str_unsafe("my-operator");
        let controller_name = ControllerName::from_str_unsafe("my-controller");
        let role_name = RoleName::from_str_unsafe("my-role");

        let role_labels = recommended_labels_for_role_resources(
            &cluster_name,
            &product_name,
            &product_version,
            &operator_name,
            &controller_name,
            &role_name,
        );

        let role_selector = role_selector(&cluster_name, &product_name, &role_name);

        assert!(
            role_selector
                .iter()
                .all(|selector| role_labels.contains(selector))
        );
    }

    #[test]
    fn recommended_labels_for_role_group_resources_produces_expected_labels() {
        let actual_labels = recommended_labels_for_role_group_resources(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &ProductVersion::from_str_unsafe("1.0.0"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
            &RoleName::from_str_unsafe("my-role"),
            &RoleGroupName::from_str_unsafe("my-role-group"),
        );

        let expected_labels: BTreeMap<_, _> = [
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
    fn recommended_labels_for_unversioned_role_group_resources_produces_expected_labels() {
        let actual_labels = recommended_labels_for_unversioned_role_group_resources(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &OperatorName::from_str_unsafe("my-operator"),
            &ControllerName::from_str_unsafe("my-controller"),
            &RoleName::from_str_unsafe("my-role"),
            &RoleGroupName::from_str_unsafe("my-role-group"),
        );

        let expected_labels: BTreeMap<_, _> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/managed-by", "my-operator_my-controller"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/role-group", "my-role-group"),
            ("stackable.tech/vendor", "Stackable"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn role_group_selector_produces_expected_labels() {
        let actual_labels = role_group_selector(
            &ClusterName::from_str_unsafe("cluster-name"),
            &ProductName::from_str_unsafe("my-product"),
            &RoleName::from_str_unsafe("my-role"),
            &RoleGroupName::from_str_unsafe("my-role-group"),
        );

        let expected_labels: BTreeMap<_, _> = [
            ("app.kubernetes.io/component", "my-role"),
            ("app.kubernetes.io/instance", "cluster-name"),
            ("app.kubernetes.io/name", "my-product"),
            ("app.kubernetes.io/role-group", "my-role-group"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();

        assert_eq!(expected_labels, actual_labels.into());
    }

    #[test]
    fn role_group_selector_is_subset_of_recommended_role_group_labels() {
        let cluster_name = ClusterName::from_str_unsafe("cluster-name");
        let product_name = ProductName::from_str_unsafe("my-product");
        let product_version = ProductVersion::from_str_unsafe("1.0.0");
        let operator_name = OperatorName::from_str_unsafe("my-operator");
        let controller_name = ControllerName::from_str_unsafe("my-controller");
        let role_name = RoleName::from_str_unsafe("my-role");
        let role_group_name = RoleGroupName::from_str_unsafe("my-role-group");

        let role_group_labels = recommended_labels_for_role_group_resources(
            &cluster_name,
            &product_name,
            &product_version,
            &operator_name,
            &controller_name,
            &role_name,
            &role_group_name,
        );

        let unversioned_role_group_labels = recommended_labels_for_unversioned_role_group_resources(
            &cluster_name,
            &product_name,
            &operator_name,
            &controller_name,
            &role_name,
            &role_group_name,
        );

        let role_group_selector =
            role_group_selector(&cluster_name, &product_name, &role_name, &role_group_name);

        assert!(
            role_group_selector
                .iter()
                .all(|selector| role_group_labels.contains(selector))
        );

        assert!(
            role_group_selector
                .iter()
                .all(|selector| unversioned_role_group_labels.contains(selector))
        );
    }
}
