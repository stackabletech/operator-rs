use std::str::FromStr;

use crate::{cli::ProductOperatorRun, commons::networking::DomainName};

const KUBERNETES_CLUSTER_DOMAIN_DEFAULT: &str = "cluster.local";

#[derive(Debug, Clone)]
pub struct KubernetesClusterInfo {
    pub cluster_domain: DomainName,
}

impl KubernetesClusterInfo {
    pub fn new(cli_opts: &ProductOperatorRun) -> Self {
        let cluster_domain = match &cli_opts.kubernetes_cluster_domain {
            Some(cluster_domain) => {
                tracing::info!(%cluster_domain, "Using configured Kubernetes cluster domain");

                cluster_domain.clone()
            }
            None => {
                // TODO(sbernauer): Do some sort of advanced auto-detection, see https://github.com/stackabletech/issues/issues/436.
                // There have been attempts of parsing the `/etc/resolv.conf`, but they have been
                // reverted. Please read on the linked Issue for details.
                let cluster_domain = DomainName::from_str(KUBERNETES_CLUSTER_DOMAIN_DEFAULT)
                    .expect("KUBERNETES_CLUSTER_DOMAIN_DEFAULT constant must a valid domain");
                tracing::info!(%cluster_domain, "Defaulting Kubernetes cluster domain as it has not been configured");

                cluster_domain
            }
        };

        Self { cluster_domain }
    }
}
