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
}
