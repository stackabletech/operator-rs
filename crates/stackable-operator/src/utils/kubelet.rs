use http;
use kube::client::Client;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};

use crate::commons::networking::DomainName;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to build request for url path \"{url_path}\""))]
    BuildConfigzRequest {
        source: http::Error,
        url_path: String,
    },

    #[snafu(display("failed to fetch kubelet config from node {node:?}"))]
    FetchNodeKubeletConfig { source: kube::Error, node: String },

    #[snafu(display("failed to fetch `kubeletconfig` JSON key from configz response"))]
    KubeletConfigJsonKey,

    #[snafu(display("failed to deserialize kubelet config JSON"))]
    KubeletConfigJson { source: serde_json::Error },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProxyConfigResponse {
    kubeletconfig: KubeletConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeletConfig {
    pub cluster_domain: DomainName,
}

impl KubeletConfig {
    /// Fetches the kubelet configuration from the specified node in the Kubernetes cluster.
    pub async fn fetch(client: &Client, node_name: &str) -> Result<Self, Error> {
        let url_path = format!("/api/v1/nodes/{node_name}/proxy/configz");
        let req = http::Request::get(url_path.clone())
            .body(Default::default())
            .context(BuildConfigzRequestSnafu { url_path })?;

        let resp = client
            .request::<ProxyConfigResponse>(req)
            .await
            .context(FetchNodeKubeletConfigSnafu { node: node_name })?;

        Ok(resp.kubeletconfig)
    }
}
