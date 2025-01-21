use std::str::FromStr;

use crate::commons::networking::DomainName;

const KUBERNETES_CLUSTER_DOMAIN_DEFAULT: &str = "cluster.local";

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
    pub fn new(cluster_info_opts: &KubernetesClusterInfoOpts) -> Self {
        let cluster_domain = match &cluster_info_opts.kubernetes_cluster_domain {
            Some(cluster_domain) => {
                tracing::info!(%cluster_domain, "Using configured Kubernetes cluster domain");

                cluster_domain.clone()
            }
            None => {
                // TODO(sbernauer): Do some sort of advanced auto-detection, see https://github.com/stackabletech/issues/issues/436.
                // There have been attempts of parsing the `/etc/resolv.conf`, but they have been
                // reverted. Please read on the linked issue for details.
                let cluster_domain = DomainName::from_str(KUBERNETES_CLUSTER_DOMAIN_DEFAULT)
                    .expect("KUBERNETES_CLUSTER_DOMAIN_DEFAULT constant must a valid domain");
                tracing::info!(%cluster_domain, "Defaulting Kubernetes cluster domain as it has not been configured");

                cluster_domain
            }
        };

        Self { cluster_domain }
    }
}
