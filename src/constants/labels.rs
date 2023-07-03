use const_format::concatcp;

pub const APP_KUBERNETES_LABEL_BASE: &str = "app.kubernetes.io/";

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
