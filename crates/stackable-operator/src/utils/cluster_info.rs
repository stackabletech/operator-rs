use kube::Client;
use snafu::{ResultExt, Snafu};

use crate::{commons::networking::DomainName, utils::kubelet};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("unable to fetch kubelet config"))]
    KubeletConfig { source: kubelet::Error },
}

#[derive(Debug, Clone)]
pub struct KubernetesClusterInfo {
    /// The Kubernetes cluster domain, typically `cluster.local`.
    pub cluster_domain: DomainName,
}

#[cfg_attr(
    feature = "clap",
    derive(clap::Parser),
    command(next_help_heading = "Cluster Options")
)]
#[derive(Debug, PartialEq, Eq)]
pub struct KubernetesClusterInfoOptions {
    /// Kubernetes cluster domain, usually this is `cluster.local`.
    // We are not using a default value here, as we query the cluster if it is not specified.
    #[cfg_attr(feature = "clap", arg(long, env))]
    pub kubernetes_cluster_domain: Option<DomainName>,

    /// Name of the Kubernetes Node that the operator is running on.
    ///
    /// Note that when running the operator on Kubernetes we recommend to use the
    /// [downward API](https://kubernetes.io/docs/concepts/workloads/pods/downward-api/)
    /// to let Kubernetes project the namespace as the `KUBERNETES_NODE_NAME` env variable.
    #[cfg_attr(feature = "clap", arg(long, env))]
    pub kubernetes_node_name: String,
}

impl KubernetesClusterInfo {
    pub async fn new(
        client: &Client,
        cluster_info_opts: &KubernetesClusterInfoOptions,
    ) -> Result<Self, Error> {
        let cluster_domain = match cluster_info_opts {
            KubernetesClusterInfoOptions {
                kubernetes_cluster_domain: Some(cluster_domain),
                ..
            } => {
                tracing::info!(%cluster_domain, "Using configured Kubernetes cluster domain");

                cluster_domain.clone()
            }
            KubernetesClusterInfoOptions {
                kubernetes_node_name: node_name,
                ..
            } => {
                tracing::info!(%node_name, "Fetching Kubernetes cluster domain from the local kubelet");
                let kubelet_config = kubelet::KubeletConfig::fetch(client, node_name)
                    .await
                    .context(KubeletConfigSnafu)?;

                let cluster_domain = kubelet_config.cluster_domain;
                tracing::info!(%cluster_domain, "Using Kubernetes cluster domain from the kubelet config");

                cluster_domain
            }
        };

        Ok(Self { cluster_domain })
    }
}
