use crate::utils::format_full_controller_name;
use const_format::concatcp;
use kube::api::{Resource, ResourceExt};
use std::collections::BTreeMap;

#[cfg(doc)]
use crate::builder::ObjectMetaBuilder;
#[cfg(doc)]
use crate::commons::product_image_selection::ResolvedProductImage;

const APP_KUBERNETES_LABEL_BASE: &str = "app.kubernetes.io/";

/// The name of the application e.g. "mysql"
pub const APP_NAME_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "name");
/// A unique name identifying the instance of an application e.g. "mysql-abcxzy"
pub const APP_INSTANCE_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "instance");
/// The current version of the application (e.g., a semantic version, revision hash, etc.) e.g."5.7.21"
pub const APP_VERSION_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "version");
/// The component within the architecture e.g. database
pub const APP_COMPONENT_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "component");
/// The name of a higher level application this one is part of e.g. "wordpress"
pub const APP_PART_OF_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "part-of");
/// The tool being used to manage the operation of an application e.g. helm
pub const APP_MANAGED_BY_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "managed-by");
pub const APP_ROLE_GROUP_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "role-group");

/// Recommended labels to set on objects created by Stackable operators
///
/// See [`get_recommended_labels`] and [`ObjectMetaBuilder::with_recommended_labels`].
#[derive(Debug, Clone, Copy)]
pub struct ObjectLabels<'a, T> {
    /// The name of the object that this object is being created on behalf of, such as a `ZookeeperCluster`
    pub owner: &'a T,
    /// The name of the app being managed, such as `zookeeper`
    pub app_name: &'a str,
    /// The version of the app being managed (not of the operator).
    ///
    /// If setting this label on a Stackable product please use the provided `app_version_label` attribute of the
    /// [`ResolvedProductImage`] struct
    ///
    /// This version should include the Stackable version, such as `3.0.0-stackable0.1.0`.
    /// If the Stackable version is not known, the product version should be used together with a suffix (if possible).
    /// In the case of custom product images provided by the user, only the product version is known,
    /// `3.0.0-<tag-of-custom-image>` should be used.
    ///
    /// However, this is pure documentation and should not be parsed.
    pub app_version: &'a str,
    /// The DNS-style name of the operator managing the object (such as `zookeeper.stackable.tech`)
    pub operator_name: &'a str,
    /// The name of the controller inside of the operator managing the object (such as `zookeepercluster`)
    pub controller_name: &'a str,
    /// The role that this object belongs to
    pub role: &'a str,
    /// The role group that this object belongs to
    pub role_group: &'a str,
}

/// Create kubernetes recommended labels
pub fn get_recommended_labels<T>(
    ObjectLabels {
        owner,
        app_name,
        app_version,
        operator_name,
        controller_name,
        role,
        role_group,
    }: ObjectLabels<T>,
) -> BTreeMap<String, String>
where
    T: Resource,
{
    let mut labels = role_group_selector_labels(owner, app_name, role, role_group);

    // TODO: Add operator version label
    // TODO: part-of is empty for now, decide on how this can be used in a proper fashion
    labels.insert(APP_VERSION_LABEL.to_string(), app_version.to_string());
    labels.insert(
        APP_MANAGED_BY_LABEL.to_string(),
        format_full_controller_name(operator_name, controller_name),
    );

    labels
}

/// The labels required to match against objects of a certain role, assuming that those objects
/// are defined using [`get_recommended_labels`]
pub fn role_group_selector_labels<T: Resource>(
    owner: &T,
    app_name: &str,
    role: &str,
    role_group: &str,
) -> BTreeMap<String, String> {
    let mut labels = role_selector_labels(owner, app_name, role);
    labels.insert(APP_ROLE_GROUP_LABEL.to_string(), role_group.to_string());
    labels
}

/// The labels required to match against objects of a certain role group, assuming that those objects
/// are defined using [`get_recommended_labels`]
pub fn role_selector_labels<T: Resource>(
    owner: &T,
    app_name: &str,
    role: &str,
) -> BTreeMap<String, String> {
    let mut labels = build_common_labels_for_all_managed_resources(app_name, &owner.name_any());
    labels.insert(APP_COMPONENT_LABEL.to_string(), role.to_string());
    labels
}

/// The APP_NAME_LABEL (Spark, Kafka, ZooKeeper...) and APP_INSTANCES_LABEL (simple, test ...) are
/// required to identify resources that belong to a certain owner object (such as a particular `ZookeeperCluster`).
pub fn build_common_labels_for_all_managed_resources(
    app_name: &str,
    owner_name: &str,
) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert(APP_NAME_LABEL.to_string(), app_name.to_string());
    labels.insert(APP_INSTANCE_LABEL.to_string(), owner_name.to_string());
    labels
}
