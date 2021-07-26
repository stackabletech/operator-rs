use crate::product_config_utils;
use std::collections::HashSet;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to serialize template to JSON: {source}")]
    JsonSerializationError {
        #[from]
        source: serde_json::Error,
    },

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

    #[error("Object is missing key: {key}")]
    MissingObjectKey { key: &'static str },

    #[error("LabelSelector is invalid: {message}")]
    InvalidLabelSelector { message: String },

    #[error("CustomResource [{name}] not found in any 'metadata.name' field. Could not retrieve OwnerReference.")]
    MissingCustomResource { name: String },

    #[error("OwnerReference for command [{command}] with owner [{owner}] is missing.")]
    MissingOwnerReference { command: String, owner: String },

    #[error("Role [{role}] is missing. This should not happen. Will requeue.")]
    MissingRole { role: String },

    #[error("RoleGroup [{role_group}] for Role [{role}] is missing. This may happen after custom resource changes. Will requeue.")]
    MissingRoleGroup { role: String, role_group: String },

    #[error("Operation timed out: {source}")]
    TimeoutError {
        #[from]
        source: tokio::time::error::Elapsed,
    },

    #[error("Environment variable error: {source}")]
    EnvironmentVariableError {
        #[from]
        source: std::env::VarError,
    },

    #[error("Invalid name for resource: {errors:?}")]
    InvalidName { errors: Vec<String> },

    #[error("The following required CRDs are missing from Kubernetes: {names:?}")]
    RequiredCrdsMissing { names: HashSet<String> },

    #[error(
        "A required File is missing. Not found in any of the following locations: {search_path:?}"
    )]
    RequiredFileMissing { search_path: Vec<String> },

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
    FromUtf8Error(#[from] std::string::FromUtf8Error),
}

pub type OperatorResult<T> = std::result::Result<T, Error>;
