use const_format::concatcp;

/// The well-known Kubernetes app key prefix.
const K8S_APP_KEY_PREFIX: &str = "app.kubernetes.io/";

/// The Stackable-specific general key prefix.
const STACKABLE_KEY_PREFIX: &str = "stackable.tech/";

/// The well-known Kubernetes app name key `app.kubernetes.io/name`. It is used
/// to label the application with a name, e.g. `mysql`.
pub const K8S_APP_NAME_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "name");

/// The well-known Kubernetes app instance key `app.kubernetes.io/instance`. It
/// is used to identify the instance of an application, e.g. `mysql-abcxyz`.
pub const K8S_APP_INSTANCE_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "instance");

/// The well-known Kubernetes app version key `app.kubernetes.io/version`. It is
/// used to indicate the current version of the application. The value can
/// represent a semantic version or a revision, e.g. `5.7.21`.
pub const K8S_APP_VERSION_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "version");

/// The well-known Kubernetes app component key `app.kubernetes.io/component`.
/// It is used to specify the compoent within the architecture, e.g. `database`.
pub const K8S_APP_COMPONENT_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "component");

/// The well-known Kubernetes app part-of key `app.kubernetes.io/part-of`. It is
/// used to specify the name of a higher level application this one is part of,
/// e.g. `wordpress`.
pub const K8S_APP_PART_OF_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "part-of");

/// The well-known Kubernetes app managed-by key `app.kubernetes.io/managed-by`.
/// It is used to indicate what tool is being used to manage the operation of
/// an application, e.g. `helm`.
pub const K8S_APP_MANAGED_BY_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "managed-by");

/// The well-kown Kubernetes app role-group key `app.kubernetes.io/role-group`.
/// It is used to specify to which role group this application belongs to, e.g.
/// `worker`.
pub const K8S_APP_ROLE_GROUP_KEY: &str = concatcp!(K8S_APP_KEY_PREFIX, "role-group");

/// The common Stackable vendor key `stackable.tech/vendor`. It is used to
/// indicate that the resource was deployed as part of the SDP.
pub const STACKABLE_VENDOR_KEY: &str = concatcp!(STACKABLE_KEY_PREFIX, "vendor");
