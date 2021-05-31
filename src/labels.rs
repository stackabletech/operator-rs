use const_format::concatcp;
use kube::Resource;
use std::collections::BTreeMap;

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

/// Create kubernetes recommended labels:
/// - app.kubernetes.io/instance
pub fn get_recommended_labels<T>(
    resource: &T,
    app_name: &str,
    app_version: &str,
    app_component: &str,
    role_name: &str,
) -> BTreeMap<String, String>
where
    T: Resource,
{
    let mut recommended_labels = BTreeMap::new();

    // TODO: Add operator version label
    // TODO: part-of is empty for now, decide on how this can be used in a proper fashion
    recommended_labels.insert(APP_INSTANCE_LABEL.to_string(), resource.name());
    recommended_labels.insert(APP_NAME_LABEL.to_string(), app_name.to_string());
    recommended_labels.insert(APP_VERSION_LABEL.to_string(), app_version.to_string());
    recommended_labels.insert(APP_COMPONENT_LABEL.to_string(), app_component.to_string());
    recommended_labels.insert(APP_ROLE_GROUP_LABEL.to_string(), role_name.to_string());
    recommended_labels.insert(
        APP_MANAGED_BY_LABEL.to_string(),
        format!("{}-operator", app_name),
    );

    recommended_labels
}
