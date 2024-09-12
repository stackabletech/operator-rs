use serde::Deserialize;
use snafu::{ResultExt, Snafu};

use crate::client::Client;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse list of supported Kubernetes versions"))]
    ParseSupportedKubernetesVersions { source: serde_json::Error },

    #[snafu(display("failed to determine the current Kubernetes versions"))]
    DetermineCurrentKubernetesVersion { source: kube::Error },
}

/// [`k8s_openapi::apimachinery::pkg::version::Info`] tracks these fields as Strings, so let's stick to that
#[derive(Debug, Deserialize, PartialEq)]
struct K8sVersion {
    major: String,
    minor: String,
}

pub async fn warn_if_unsupported_k8s_version(
    client: &Client,
    supported_versions_json: &str,
) -> Result<(), Error> {
    let supported_versions: Vec<K8sVersion> = serde_json::from_str(supported_versions_json)
        .context(ParseSupportedKubernetesVersionsSnafu)?;

    let current_version = client
        .as_kube_client()
        .apiserver_version()
        .await
        .context(DetermineCurrentKubernetesVersionSnafu)?;

    let current_version = K8sVersion {
        major: current_version.major,
        minor: current_version.minor,
    };

    if !supported_versions.contains(&current_version) {
        tracing::warn!(
            ?current_version,
            ?supported_versions,
            "You are running an unsupported Kubernetes version. Things might work - but are not guaranteed to! Please consider switching to a supported Kubernetes version"
        );
    }

    Ok(())
}
