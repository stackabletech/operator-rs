use crate::product_config_utils;
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

    #[error("Object is missing key: {key}")]
    MissingObjectKey { key: &'static str },

    #[error("LabelSelector is invalid: {message}")]
    InvalidLabelSelector { message: String },

    #[error("Role [{role}] is missing. This should not happen. Will requeue.")]
    MissingRole { role: String },

    #[error("RoleGroup [{role_group}] for Role [{role}] is missing. This may happen after custom resource changes. Will requeue.")]
    MissingRoleGroup { role: String, role_group: String },

    #[error("Environment variable error: {source}")]
    EnvironmentVariableError {
        #[from]
        source: std::env::VarError,
    },

    #[error("Invalid name for resource: {errors:?}")]
    InvalidName { errors: Vec<String> },

    #[error(
        "A required File is missing. Not found in any of the following locations: {search_path:?}"
    )]
    RequiredFileMissing { search_path: Vec<PathBuf> },

    #[error("Failed to load ProductConfig: {source}")]
    ProductConfigLoadError {
        #[source]
        source: product_config::error::Error,
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
}

pub type OperatorResult<T> = std::result::Result<T, Error>;
