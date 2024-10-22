use std::{env, str::FromStr};

use snafu::{ResultExt, Snafu};
use tracing::instrument;

use crate::commons::networking::DomainName;

const KUBERNETES_CLUSTER_DOMAIN_ENV: &str = "KUBERNETES_CLUSTER_DOMAIN";
const KUBERNETES_CLUSTER_DOMAIN_DEFAULT: &str = "cluster.local";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse {cluster_domain:?} as domain name"))]
    ParseDomainName {
        source: crate::validation::Errors,
        cluster_domain: String,
    },
}

/// Tries to retrieve the Kubernetes cluster domain.
///
/// Return `KUBERNETES_CLUSTER_DOMAIN` if set, otherwise default to
/// [`KUBERNETES_CLUSTER_DOMAIN_DEFAULT`].
#[instrument]
pub(crate) fn retrieve_cluster_domain() -> Result<DomainName, Error> {
    tracing::debug!("Trying to determine the Kubernetes cluster domain...");

    Ok(match env::var(KUBERNETES_CLUSTER_DOMAIN_ENV) {
        Ok(cluster_domain) if !cluster_domain.is_empty() => {
            let cluster_domain = DomainName::from_str(&cluster_domain)
                .context(ParseDomainNameSnafu { cluster_domain })?;
            tracing::info!(
                %cluster_domain,
                "Using Kubernetes cluster domain from {KUBERNETES_CLUSTER_DOMAIN_ENV:?} environment variable"
            );
            cluster_domain
        }
        _ => {
            let cluster_domain = DomainName::from_str(KUBERNETES_CLUSTER_DOMAIN_DEFAULT)
                .expect("KUBERNETES_CLUSTER_DOMAIN_DEFAULT constant must a valid domain");
            tracing::info!(
                %cluster_domain,
                "Using default Kubernetes cluster domain as {KUBERNETES_CLUSTER_DOMAIN_ENV:?} environment variable is not set"
            );
            cluster_domain
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_kubernetes_cluster_domain_value() {
        assert_eq!(
            retrieve_cluster_domain().unwrap().to_string(),
            "cluster.local"
        );
    }

    #[test]
    fn use_different_kubernetes_cluster_domain_value() {
        let cluster_domain = "my-cluster.local";

        // Set custom cluster domain via env var
        unsafe {
            env::set_var(KUBERNETES_CLUSTER_DOMAIN_ENV, cluster_domain);
        }

        assert_eq!(
            retrieve_cluster_domain().unwrap().to_string(),
            cluster_domain
        );
    }
}
