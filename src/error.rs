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
}

pub type OperatorResult<T> = std::result::Result<T, Error>;
