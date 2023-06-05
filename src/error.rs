use crate::{commons::resources::ResourceRequirementsError, product_config_utils};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to serialize YAML: {source}")]
    YamlSerializationError {
        #[from]
        source: serde_yaml::Error,
    },

    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },

    #[error("Kubernetes failed to delete object: {source}")]
    KubeDeleteError {
        #[from]
        source: kube::runtime::wait::delete::Error,
    },

    #[error("Object is missing key: {key}")]
    MissingObjectKey { key: &'static str },

    #[error("Label is missing: {label}")]
    MissingLabel { label: &'static str },

    #[error(
        "Label contains unexpected content. \
            Expected: {label}={expected_content}, \
            actual: {label}={actual_content}"
    )]
    UnexpectedLabelContent {
        label: &'static str,
        expected_content: String,
        actual_content: String,
    },

    #[error("LabelSelector is invalid: {message}")]
    InvalidLabelSelector { message: String },

    #[error("Role [{role}] is missing. This should not happen. Will requeue.")]
    MissingRole { role: String },

    #[error("RoleGroup [{role_group}] for Role [{role}] is missing. This may happen after custom resource changes. Will requeue.")]
    MissingRoleGroup { role: String, role_group: String },

    #[error(
        "A required File is missing. Not found in any of the following locations: {search_path:?}"
    )]
    RequiredFileMissing { search_path: Vec<PathBuf> },

    #[error("Failed to load ProductConfig: {source}")]
    ProductConfigLoadError {
        #[source]
        source: Box<product_config::error::Error>,
    },

    #[error("ProductConfig Framework reported error: {source}")]
    ProductConfigError {
        #[from]
        source: product_config_utils::ConfigError,
    },

    #[error("IO Error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("Error converting CRD byte array to UTF-8")]
    CrdFromUtf8Error(#[source] std::string::FromUtf8Error),

    #[error("Missing OPA connect string in configmap [{configmap_name}]")]
    MissingOpaConnectString { configmap_name: String },

    #[error("Missing S3 connection [{name}]")]
    MissingS3Connection { name: String },

    #[error("Missing S3 bucket [{name}]")]
    MissingS3Bucket { name: String },

    #[error("Invalid quantity [{value}]")]
    InvalidQuantity { value: String },

    #[error("Invalid cpu quantity [{value}]")]
    InvalidCpuQuantity { value: String },

    #[error("Invalid quantity unit [{value}]")]
    InvalidQuantityUnit { value: String },

    #[error("No quantity unit provided for [{value}]")]
    NoQuantityUnit { value: String },

    #[error("Unsupported Precision [{value}]. Kubernetes doesn't allow you to specify CPU resources with a precision finer than 1m. Because of this, it's useful to specify CPU units less than 1.0 or 1000m using the milliCPU form; for example, 5m rather than 0.005")]
    UnsupportedCpuQuantityPrecision { value: String },

    #[error("Cannot scale down from kilobytes.")]
    CannotScaleDownMemoryUnit,

    #[error("Cannot convert quantity [{value}] to Java heap.")]
    CannotConvertToJavaHeap { value: String },

    #[error("Cannot convert quantity [{value}] to Java heap value with unit [{target_unit}].")]
    CannotConvertToJavaHeapValue { value: String, target_unit: String },

    #[error("container name {container_name:?} is invalid: {violation}")]
    InvalidContainerName {
        container_name: String,
        violation: String,
    },

    /// This error indicates that a resource policy is missing for a container.
    #[error("resource requirements policy error: {0}")]
    MissingResourceRequirementType(#[from] ResourceRequirementsError),
}

pub type OperatorResult<T> = std::result::Result<T, Error>;
