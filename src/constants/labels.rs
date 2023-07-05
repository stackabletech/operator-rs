/// The label key prefix used for Kubernetes apps
pub const LABEL_KEY_PREFIX_APP_KUBERNETES: &str = "app.kubernetes.io";

/// The label key name identifying the tool used to manage the operation of an
/// application, e.g. "helm"
pub const LABEL_KEY_NAME_APP_MANAGED_BY: &str = "managed-by";

pub const LABEL_KEY_NAME_APP_ROLE_GROUP: &str = "role-group";

/// The label key name identifying the application component within the
/// architecture, e.g. "database"
pub const LABEL_KEY_NAME_APP_COMPONENT: &str = "component";

/// The label key name identifying the application instance, e.g. "mysql-abcxzy"
pub const LABEL_KEY_NAME_APP_INSTANCE: &str = "instance";

/// The label key name identifying the application version, e.g. a semantic
/// version, revision hash, etc, like "5.7.21"
pub const LABEL_KEY_NAME_APP_VERSION: &str = "version";

/// The label key name identifying the higher level application this app is part
/// of, e.g. "wordpress".
pub const LABEL_KEY_NAME_APP_PART_OF: &str = "part-of";

/// The label key name identifying the application name e.g. "mysql"
pub const LABEL_KEY_NAME_APP_NAME: &str = "name";
