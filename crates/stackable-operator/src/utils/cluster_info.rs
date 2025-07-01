use super::kubelet::KubeletConfig;
use crate::commons::networking::DomainName;

/// Some information that we know about the Kubernetes cluster.
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
    pub fn new(
        kubelet_config: &KubeletConfig,
        cluster_info_opts: &KubernetesClusterInfoOpts,
    ) -> Self {
        let cluster_domain = match &cluster_info_opts.kubernetes_cluster_domain {
            Some(cluster_domain) => {
                tracing::info!(%cluster_domain, "Using configured Kubernetes cluster domain");

                cluster_domain.clone()
            }
            None => {
                let cluster_domain = kubelet_config.cluster_domain.clone();
                tracing::info!(%cluster_domain, "Using kubelet config cluster domain");

                cluster_domain
            }
        };

        Self { cluster_domain }
    }
}
