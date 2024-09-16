use std::fmt::Display;

use itertools::Itertools;
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
#[derive(Deserialize, PartialEq)]
struct K8sVersion {
    major: String,
    minor: String,
}

impl Display for K8sVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{major}.{minor}", major = self.major, minor = self.minor)
    }
}

/// We only have this struct, so that we can implement [`Display] on it
#[derive(Deserialize, PartialEq)]
struct K8sVersionList(Vec<K8sVersion>);

impl Display for K8sVersionList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.iter().join(","))
    }
}

pub async fn warn_if_unsupported_k8s_version(
    client: &Client,
    supported_versions_json: &str,
) -> Result<(), Error> {
    let supported_versions: K8sVersionList = serde_json::from_str(supported_versions_json)
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

    if !supported_versions.0.contains(&current_version) {
        tracing::warn!(
            %current_version,
            %supported_versions,
            "You are running an unsupported Kubernetes version. Things might work - but are not guaranteed to! Please consider switching to a supported Kubernetes version"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting() {
        let version_list: K8sVersionList = serde_json::from_str(
            r#"
[
    { "major": "1", "minor": "27"},
    { "major": "1", "minor": "28"},
    { "major": "1", "minor": "29"},
    { "major": "1", "minor": "30"},
    { "major": "1", "minor": "31"}
]"#,
        )
        .expect("failed to parse k8s version list");

        assert_eq!(format!("{version_list}"), "1.27,1.28,1.29,1.30,1.31");
    }
}
