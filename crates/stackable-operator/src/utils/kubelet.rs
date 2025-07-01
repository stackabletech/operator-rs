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
    #[snafu(display("failed to build proxy/configz request"))]
    ConfigzRequest { source: http::Error },

    #[snafu(display("failed to fetch kubelet config from node {node}"))]
    FetchNodeKubeletConfig { source: kube::Error, node: String },

    #[snafu(display("failed to fetch `kubeletconfig` JSON key from configz response"))]
    KubeletConfigJsonKey,

    #[snafu(display("failed to deserialize kubelet config JSON"))]
    KubeletConfigJson { source: serde_json::Error },

    #[snafu(display("empty Kubernetes nodes list"))]
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

        let name = node.name_any();

        // Query kukbelet config
        let url = format!("/api/v1/nodes/{}/proxy/configz", name);
        let req = http::Request::get(url)
            .body(Default::default())
            .context(ConfigzRequestSnafu)?;

        let resp = client
            .request::<ProxyConfigResponse>(req)
            .await
            .context(FetchNodeKubeletConfigSnafu { node: name })?;

        Ok(resp.kubeletconfig)
    }
}
