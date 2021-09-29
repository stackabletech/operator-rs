use crate::name_utils;
use crate::product_config_utils;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
    "The configmap is missing a generated name. This is a programming error. Please open a ticket."
    )]
    ConfigMapMissingGenerateName,

    #[error(
    "The config map [{name}] is missing labels [:?labels]. This is a programming error. Please open a ticket."
    )]
    ConfigMapMissingLabels {
        name: String,
        labels: Vec<&'static str>,
    },

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

    #[error("NameUtils reported error: {source}")]
    NamingError {
        #[from]
        source: name_utils::Error,
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
    #[error(
    "Not enough nodes [{number_of_nodes}] available to schedule pods [{number_of_pods}]. Unscheduled pods: {unscheduled_pods:?}."
    )]
    NotEnoughNodesAvailable {
        number_of_nodes: usize,
        number_of_pods: usize,
        unscheduled_pods: Vec<String>,
    },

    #[error(
        "PodIdentity could not be parsed: [{pod_id}]. This should not happen. Please open a ticket."
    )]
    PodIdentityNotParseable { pod_id: String },

    #[error("Cannot build PodIdentity from Pod without labels. Missing labels: {0:?}")]
    PodWithoutLabelsNotSupported(Vec<String>),

    #[error("Cannot build NodeIdentity from node without name.")]
    NodeWithoutNameNotSupported,

    #[error("Cannot construct PodIdentity from empty id field.")]
    PodIdentityFieldEmpty,

    #[error(
        "Pod identity field [{field}] with value [{value}] does not match the expected value [{expected}]"
    )]
    UnexpectedPodIdentityField {
        field: String,
        value: String,
        expected: String,
    },

    #[error("Forbidden separator [{separator}] found in pod identity fields [{invalid_fields:?}]")]
    PodIdentityFieldWithInvalidSeparator {
        separator: String,
        invalid_fields: BTreeMap<String, String>,
    },

    #[error("Conversion error: [message]")]
    ConversionError { message: String },
}

pub type OperatorResult<T> = std::result::Result<T, Error>;
