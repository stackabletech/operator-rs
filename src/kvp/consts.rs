use const_format::concatcp;

const K8S_APP_KEY_PREFIX: &str = "app.kubernetes.io/";

/// The name of the application e.g. "mysql"
pub const NAME_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "name");

/// A unique name identifying the instance of an application e.g. "mysql-abcxzy"
pub const INSTANCE_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "instance");

/// The current version of the application (e.g., a semantic version, revision hash, etc.) e.g."5.7.21"
pub const VERSION_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "version");

/// The component within the architecture e.g. database
pub const COMPONENT_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "component");

/// The name of a higher level application this one is part of e.g. "wordpress"
pub const PART_OF_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "part-of");

/// The tool being used to manage the operation of an application e.g. helm
pub const MANAGED_BY_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "managed-by");

pub const ROLE_GROUP_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "role-group");
