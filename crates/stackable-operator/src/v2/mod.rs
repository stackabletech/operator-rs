use crate::v2::types::kubernetes::Uid;

pub mod builder;
pub mod cluster_resources;
pub mod config_file_writer;
pub mod config_overrides;
pub mod controller_utils;
pub mod flask_config_writer;
pub mod jvm_argument_overrides;
pub mod kvp;
pub mod macros;
pub mod product_logging;
pub mod role_group_utils;
pub mod role_utils;
pub mod types;

/// Has a non-empty name
///
/// Useful as an object reference; Should not be used to create an object because the name could
/// violate the naming constraints (e.g. maximum length) of the object.
pub trait HasName {
    fn to_name(&self) -> String;
}

/// Has a Kubernetes UID
pub trait HasUid {
    fn to_uid(&self) -> Uid;
}

/// The name is a valid label value
pub trait NameIsValidLabelValue {
    fn to_label_value(&self) -> String;
}
