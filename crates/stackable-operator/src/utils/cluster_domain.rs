use std::{env, path::Path, str::FromStr, sync::OnceLock};

use snafu::{OptionExt, ResultExt, Snafu};

use crate::commons::networking::DomainName;

const KUBERNETES_CLUSTER_DOMAIN_ENV: &str = "KUBERNETES_CLUSTER_DOMAIN";
const KUBERNETES_SERVICE_HOST_ENV: &str = "KUBERNETES_SERVICE_HOST";

const KUBERNETES_CLUSTER_DOMAIN_DEFAULT: &str = "cluster.local";
const RESOLVE_CONF_FILE_PATH: &str = "/etc/resolv.conf";

// TODO (@Techassi): Do we even need this many variants? Can we get rid of a bunch of variants and
// fall back to defaults instead? Also trace the errors
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to read resolv.conf"))]
    ReadResolvConfFile { source: std::io::Error },

    #[snafu(display("failed to parse {cluster_domain:?} as domain name"))]
    ParseDomainName {
        source: crate::validation::Errors,
        cluster_domain: String,
    },

    #[snafu(display("unable to find \"search\" entry"))]
    NoSearchEntry,

    #[snafu(display("unable to find unambiguous domain in \"search\" entry"))]
    AmbiguousDomainEntries,
}

/// Tries to retrieve the Kubernetes cluster domain.
///
/// 1. Return `KUBERNETES_CLUSTER_DOMAIN` if set, otherwise
/// 2. Return the cluster domain parsed from the `/etc/resolv.conf` file if `KUBERNETES_SERVICE_HOST`
///    is set, otherwise fall back to `cluster.local`. cluster.
///
/// This variable is initialized in [`crate::client::initialize_operator`], which is called in the
/// main function. It can be used as suggested below.
///
/// ## Usage
///
/// ```no_run
/// use stackable_operator::utils::KUBERNETES_CLUSTER_DOMAIN;
///
/// let kubernetes_cluster_domain = KUBERNETES_CLUSTER_DOMAIN.get()
///     .expect("KUBERNETES_CLUSTER_DOMAIN must first be set by calling initialize_operator");
///
/// tracing::info!(%kubernetes_cluster_domain, "Found cluster domain");
/// ```
///
/// ## See
///
/// - <https://github.com/stackabletech/issues/issues/436>
/// - <https://kubernetes.io/docs/concepts/services-networking/dns-pod-service/>
pub static KUBERNETES_CLUSTER_DOMAIN: OnceLock<DomainName> = OnceLock::new();

pub(crate) fn retrieve_cluster_domain() -> Result<DomainName, Error> {
    // 1. Read KUBERNETES_CLUSTER_DOMAIN env var
    tracing::info!("Trying to determine the Kubernetes cluster domain...");

    match env::var(KUBERNETES_CLUSTER_DOMAIN_ENV) {
        Ok(cluster_domain) if !cluster_domain.is_empty() => {
            tracing::info!(cluster_domain, "Kubernetes cluster domain set by environment variable");
            return DomainName::from_str(&cluster_domain)
                .context(ParseDomainNameSnafu { cluster_domain });
        }
        _ => {
            tracing::info!("The env var '{KUBERNETES_CLUSTER_DOMAIN_ENV}' is not set or empty");
        }
    };

    // 2. If no env var is set, check if we run in a clustered (Kubernetes/Openshift) environment
    //    by checking if KUBERNETES_SERVICE_HOST is set: If not default to 'cluster.local'.
    tracing::info!("Trying to determine the operator runtime environment...");

    match env::var(KUBERNETES_SERVICE_HOST_ENV) {
        Ok(_) => {
            let cluster_domain = retrieve_cluster_domain_from_resolv_conf(RESOLVE_CONF_FILE_PATH)?;

            tracing::info!(
                cluster_domain,
                "Using Kubernetes cluster domain from {RESOLVE_CONF_FILE_PATH} file"
            );

            DomainName::from_str(&cluster_domain).context(ParseDomainNameSnafu { cluster_domain })
        }
        Err(_) => {
            tracing::info!(
                cluster_domain = KUBERNETES_CLUSTER_DOMAIN_DEFAULT,
                "Using default Kubernetes cluster domain"
            );
            Ok(DomainName::from_str(KUBERNETES_CLUSTER_DOMAIN_DEFAULT).expect("stuff"))
        }
    }
}

fn retrieve_cluster_domain_from_resolv_conf<P>(path: P) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    let content = std::fs::read_to_string(path).context(ReadResolvConfFileSnafu)?;

    let last = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("search"))
        .map(|l| l.trim_start_matches("search"))
        .last()
        .context(NoSearchEntrySnafu)?;

    let shortest_entry = last
        .split_ascii_whitespace()
        .min_by_key(|item| item.len())
        .context(AmbiguousDomainEntriesSnafu)?;

    // NOTE (@Techassi): This is really sad and bothers me more than I would like to admit. This
    // clone could be removed by using the code directly in the calling function. But that would
    // remove the possibility to easily test the parsing.
    Ok(shortest_entry.to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use rstest::rstest;

    #[test]
    fn use_different_kubernetes_cluster_domain_value() {
        let cluster_domain = "my-cluster.local".to_string();

        // set different domain via env var
        unsafe {
            env::set_var(KUBERNETES_CLUSTER_DOMAIN_ENV, &cluster_domain);
        }

        // initialize the lock
        let _ = KUBERNETES_CLUSTER_DOMAIN.set(retrieve_cluster_domain().unwrap());

        assert_eq!(
            cluster_domain,
            KUBERNETES_CLUSTER_DOMAIN.get().unwrap().to_string()
        );
    }

    #[rstest]
    fn parse_resolv_conf_pass(
        #[files("fixtures/cluster_domain/pass/*.resolv.conf")] path: PathBuf,
    ) {
        assert_eq!(
            retrieve_cluster_domain_from_resolv_conf(path).unwrap(),
            KUBERNETES_CLUSTER_DOMAIN_DEFAULT
        );
    }

    #[rstest]
    fn parse_resolv_conf_fail(
        #[files("fixtures/cluster_domain/fail/*.resolv.conf")] path: PathBuf,
    ) {
        assert!(retrieve_cluster_domain_from_resolv_conf(path).is_err());
    }
}
