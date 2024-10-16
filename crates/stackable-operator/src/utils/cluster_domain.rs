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

    #[snafu(display("failed to parse {cluster_domain:?} as cluster domain"))]
    ParseDomainName {
        source: crate::validation::Errors,
        cluster_domain: String,
    },

    #[snafu(display("No 'search' entries found in"))]
    SearchEntryNotFound,

    #[snafu(display("Could not trim search entry in '{search_entry_line}'."))]
    TrimSearchEntryFailed { search_entry_line: String },

    #[snafu(display("Could not find any cluster domain entry in search line."))]
    LookupClusterDomainEntryFailed,
}

/// This is the primary entry point to retrieve the Kubernetes cluster domain.
///
/// Implements the logic decided in <https://github.com/stackabletech/issues/issues/436>
///
/// 1. Check if KUBERNETES_CLUSTER_DOMAIN is set -> return if set
/// 2. Check if KUBERNETES_SERVICE_HOST is set to determine if we run in a Kubernetes / Openshift cluster
///    2.1 If set continue and parse the `resolv.conf`
///    2.2 If not set default to `cluster.local`
/// 3. Read and parse the `resolv.conf`.
///
/// # Context
///
/// This variable is initialized in [`crate::client::initialize_operator`], which is called
/// in the main function. It can be used as suggested below.
///
/// # Usage
///
/// ```no_run
/// use stackable_operator::client::{Client, initialize_operator};
/// use stackable_operator::utils::KUBERNETES_CLUSTER_DOMAIN;
///
/// #[tokio::main]
/// async fn main(){
///     let client: Client = initialize_operator(None).await.expect("Unable to construct client.");
///     let kubernetes_cluster_domain = KUBERNETES_CLUSTER_DOMAIN.get().expect("Could not resolve the Kubernetes cluster domain!");
///     tracing::info!("Found cluster domain: {kubernetes_cluster_domain}");
/// }
/// ```
pub static KUBERNETES_CLUSTER_DOMAIN: OnceLock<DomainName> = OnceLock::new();

pub(crate) fn retrieve_cluster_domain() -> Result<DomainName, Error> {
    // 1. Read KUBERNETES_CLUSTER_DOMAIN env var
    tracing::info!("Trying to determine the Kubernetes cluster domain...");

    match env::var(KUBERNETES_CLUSTER_DOMAIN_ENV) {
        Ok(cluster_domain) if !cluster_domain.is_empty() => {
            tracing::info!("Using Kubernetes cluster domain: '{cluster_domain}'");
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
        .context(SearchEntryNotFoundSnafu)?;

    let shortest_entry = last
        .split_ascii_whitespace()
        .min_by_key(|item| item.len())
        .context(LookupClusterDomainEntryFailedSnafu)?;

    // NOTE (@Techassi): This is really sad and bothers me more than I would like to admit
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
