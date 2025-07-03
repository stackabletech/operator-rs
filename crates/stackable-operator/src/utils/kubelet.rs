use http;
use k8s_openapi::api::core::v1::Node;
use kube::{
    Api,
    api::{ListParams, ResourceExt},
    client::Client,
};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu};

use crate::commons::networking::DomainName;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to list nodes"))]
    ListNodes { source: kube::Error },

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

    #[snafu(display(
        "empty Kubernetes nodes list. At least one node is required to fetch the cluster domain from the kubelet config"
    ))]
    EmptyKubernetesNodesList,
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
    /// Fetches the kubelet configuration from the "first" node in the Kubernetes cluster.
    pub async fn fetch(client: &Client) -> Result<Self, Error> {
        let api: Api<Node> = Api::all(client.clone());
        let nodes = api
            .list(&ListParams::default())
            .await
            .context(ListNodesSnafu)?;
        let node = nodes.iter().next().context(EmptyKubernetesNodesListSnafu)?;
        let node_name = node.name_any();

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
