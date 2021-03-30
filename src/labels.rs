use const_format::concatcp;

const APP_KUBERNETES_LABEL_BASE: &str = "app.kubernetes.io/";

pub const APP_NAME_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "name");
pub const APP_INSTANCE_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "instance");
pub const APP_VERSION_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "version");
pub const APP_COMPONENT_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "component");
pub const APP_PART_OF_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "part-of");
pub const APP_MANAGED_BY_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "managed-by");
pub const APP_ROLE_GROUP_LABEL: &str = concatcp!(APP_KUBERNETES_LABEL_BASE, "role-group");
