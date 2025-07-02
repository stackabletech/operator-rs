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

#[derive(clap::Parser, Debug, Default, PartialEq, Eq)]
pub struct KubernetesClusterInfoOpts {
    /// Kubernetes cluster domain, usually this is `cluster.local`.
    // We are not using a default value here, as operators will probably do an more advanced
    // auto-detection of the cluster domain in case it is not specified in the future.
    #[arg(long, env)]
    pub kubernetes_cluster_domain: Option<DomainName>,
}

impl KubernetesClusterInfo {
    pub async fn new(
        client: &Client,
        cluster_info_opts: &KubernetesClusterInfoOpts,
    ) -> Result<Self, Error> {
        let cluster_domain = match &cluster_info_opts.kubernetes_cluster_domain {
            Some(cluster_domain) => {
                tracing::info!(%cluster_domain, "Using configured Kubernetes cluster domain");

                cluster_domain.clone()
            }
            None => {
                let kubelet_config = kubelet::KubeletConfig::fetch(client)
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
