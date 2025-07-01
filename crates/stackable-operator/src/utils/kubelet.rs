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
pub struct KubeletConfig {
    pub cluster_domain: DomainName,
}

impl KubeletConfig {
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

        // Deserialize JSON response as a JSON value. Alternatively, a type that
        // implements `Deserialize` can be used.
        let resp = client
            .request::<serde_json::Value>(req)
            .await
            .context(FetchNodeKubeletConfigSnafu { node: name })?;

        // Our JSON value is an object so we can treat it like a dictionary.
        let summary = resp
            .get("kubeletconfig")
            .context(KubeletConfigJsonKeySnafu)?;

        serde_json::from_value::<KubeletConfig>(summary.to_owned()).context(KubeletConfigJsonSnafu)
    }
}
