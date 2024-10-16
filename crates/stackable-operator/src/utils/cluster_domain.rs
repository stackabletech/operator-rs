use std::{
    env,
    io::{self, BufRead},
    path::Path,
    sync::OnceLock,
};

use snafu::{OptionExt, ResultExt, Snafu};

use crate::commons::networking::DomainName;

// Env vars
const KUBERNETES_CLUSTER_DOMAIN_ENV: &str = "KUBERNETES_CLUSTER_DOMAIN";
const KUBERNETES_SERVICE_HOST_ENV: &str = "KUBERNETES_SERVICE_HOST";
// Misc
const KUBERNETES_CLUSTER_DOMAIN_DEFAULT: &str = "cluster.local";
const RESOLVE_CONF_FILE_PATH: &str = "/etc/resolv.conf";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Env var '{name}' does not exist."))]
    EnvVarNotFound { source: env::VarError, name: String },

    #[snafu(display("Could not find '{resolve_conf_file_path}'."))]
    ResolvConfNotFound {
        source: io::Error,
        resolve_conf_file_path: String,
    },

    #[snafu(display("The provided cluster domain '{cluster_domain}' is not valid."))]
    InvalidDomain {
        source: crate::validation::Errors,
        cluster_domain: String,
    },

    #[snafu(display("No 'search' entries found in '{RESOLVE_CONF_FILE_PATH}'."))]
    SearchEntryNotFound { resolve_conf_file_path: String },

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

pub(crate) fn resolve_kubernetes_cluster_domain() -> Result<DomainName, Error> {
    // 1. Read KUBERNETES_CLUSTER_DOMAIN env var
    tracing::info!("Trying to determine the Kubernetes cluster domain...");
    match read_env_var(KUBERNETES_CLUSTER_DOMAIN_ENV) {
        Ok(cluster_domain) => {
            tracing::info!("Using Kubernetes cluster domain: '{cluster_domain}'");
            return cluster_domain
                .clone()
                .try_into()
                .context(InvalidDomainSnafu { cluster_domain });
        }
        Err(_) => {
            tracing::info!("The env var '{KUBERNETES_CLUSTER_DOMAIN_ENV}' is not set.");
        }
    };

    // 2. If no env var is set, check if we run in a clusterized (Kubernetes/Openshift) enviroment
    //    by checking if KUBERNETES_SERVICE_HOST is set: If not default to 'cluster.local'.
    tracing::info!("Trying to determine the operator runtime environment...");
    if read_env_var(KUBERNETES_SERVICE_HOST_ENV).is_err() {
        tracing::info!("The env var '{KUBERNETES_SERVICE_HOST_ENV}' is not set. This means we do not run in Kubernetes / Openshift. Defaulting cluster domain to '{KUBERNETES_CLUSTER_DOMAIN_DEFAULT}'.");
        return KUBERNETES_CLUSTER_DOMAIN_DEFAULT
            .to_string()
            .try_into()
            .context(InvalidDomainSnafu {
                cluster_domain: KUBERNETES_CLUSTER_DOMAIN_DEFAULT.to_string(),
            });
    }

    // 3. Read and parse 'resolv.conf'. We are looking for the last "search" entry and filter for the shortest
    //    element in that search line
    tracing::info!(
        "Running in clusterized environment. Attempting to parse '{RESOLVE_CONF_FILE_PATH}'..."
    );
    let resolve_conf_lines =
        read_file_from_path(RESOLVE_CONF_FILE_PATH).context(ResolvConfNotFoundSnafu {
            resolve_conf_file_path: RESOLVE_CONF_FILE_PATH.to_string(),
        })?;

    let cluster_domain = parse_resolve_config(resolve_conf_lines)?;
    tracing::info!("Using Kubernetes cluster domain: '{cluster_domain}'");

    cluster_domain
        .clone()
        .try_into()
        .context(InvalidDomainSnafu { cluster_domain })
}

/// Extract the Kubernetes cluster domain from the vectorized 'resolv.conf'.
/// This will:
/// 1. Use the last entry containing a 'search' prefix.
/// 2. Strip 'search' from the last entry.
/// 3. Return the shortest itme (e.g. 'cluster.local') in the whitespace seperated list.
fn parse_resolve_config(resolv_conf: Vec<String>) -> Result<String, Error> {
    tracing::debug!(
        "Start parsing '{RESOLVE_CONF_FILE_PATH}' to retrieve the Kubernetes cluster domain..."
    );

    let last_search_entry =
        find_last_search_entry(&resolv_conf).context(SearchEntryNotFoundSnafu {
            resolve_conf_file_path: RESOLVE_CONF_FILE_PATH.to_string(),
        })?;

    let last_search_entry_content =
        trim_search_line(&last_search_entry).context(TrimSearchEntryFailedSnafu {
            search_entry_line: last_search_entry.to_string(),
        })?;

    let shortest_search_entry = find_shortest_entry(last_search_entry_content)
        .context(LookupClusterDomainEntryFailedSnafu)?;

    Ok(shortest_search_entry.into())
}

/// Read an ENV variable
fn read_env_var(name: &str) -> Result<String, Error> {
    env::var(name).context(EnvVarNotFoundSnafu { name })
}

// Function to read the contents of a file and return all lines as Vec<String>
fn read_file_from_path(resolv_conf_file_path: &str) -> Result<Vec<String>, io::Error> {
    let file = std::fs::File::open(Path::new(resolv_conf_file_path))?;
    let reader = io::BufReader::new(file);

    reader.lines().collect()
}

/// Search the last entry containing the 'search' prefix. We are only interested in
/// the last line (in case there are multiple entries which would be ignored by external tools).
fn find_last_search_entry(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .rev() // Start from the end to find the last occurrence
        .find(|line| line.trim().starts_with("search"))
        .cloned()
}

/// Extract the content of the 'search' line. Basically stripping the 'search' prefix from the line like:
/// 'search sble-operators.svc.cluster.local svc.cluster.local cluster.local' will become
/// 'sble-operators.svc.cluster.local svc.cluster.local cluster.local'
fn trim_search_line(search_line: &str) -> Option<&str> {
    search_line.trim().strip_prefix("search")
}

/// Extract the shortest entry from a whitespace seperated string like:
/// 'sble-operators.svc.cluster.local svc.cluster.local cluster.local'
/// This will be 'cluster.local' here.
fn find_shortest_entry(search_content: &str) -> Option<&str> {
    search_content
        .split_whitespace()
        .min_by_key(|entry| entry.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBERNETES_RESOLV_CONF: &str = r#"""
    search sble-operators.svc.cluster.local svc.cluster.local cluster.local
    nameserver 10.243.21.53
    options ndots:5
    """#;

    const OPENSHIFT_RESOLV_CONF: &str = r#"""
    search openshift-service-ca-operator.svc.cluster.local svc.cluster.local cluster.local cmx.repl-openshift.build
    nameserver 172.30.0.10
    options ndots:5
    """#;

    const KUBERNETES_RESOLV_CONF_MULTIPLE_SEARCH_ENTRIES: &str = r#"""
    search baz svc.foo.bar foo.bar
    search sble-operators.svc.cluster.local svc.cluster.local cluster.local
    nameserver 10.243.21.53
    options ndots:5
    """#;

    const KUBERNETES_RESOLV_CONF_MISSING_SEARCH_ENTRIES: &str = r#"""
    nameserver 10.243.21.53
    options ndots:5
    """#;

    // Helper method to read resolv.conf from a string and not from file.
    fn read_file_from_string(contents: &str) -> Vec<String> {
        // Split the string by lines and collect into a Vec<String>
        contents.lines().map(|line| line.to_string()).collect()
    }

    #[test]
    fn use_different_kubernetes_cluster_domain_value() {
        let cluster_domain = "my-cluster.local".to_string();

        // set different domain via env var
        unsafe {
            env::set_var(KUBERNETES_CLUSTER_DOMAIN_ENV, &cluster_domain);
        }

        // initialize the lock
        let _ = KUBERNETES_CLUSTER_DOMAIN.set(resolve_kubernetes_cluster_domain().unwrap());

        assert_eq!(
            cluster_domain,
            KUBERNETES_CLUSTER_DOMAIN.get().unwrap().to_string()
        );
    }

    #[test]
    fn parse_resolv_conf_success() {
        let correct_resolv_configs = vec![
            KUBERNETES_RESOLV_CONF,
            OPENSHIFT_RESOLV_CONF,
            KUBERNETES_RESOLV_CONF_MULTIPLE_SEARCH_ENTRIES,
        ];

        for resolv_conf in correct_resolv_configs {
            let lines = read_file_from_string(resolv_conf);
            let last_search_entry = find_last_search_entry(lines.as_slice()).unwrap();
            let search_entry = trim_search_line(&last_search_entry).unwrap();
            let cluster_domain = find_shortest_entry(search_entry).unwrap();
            assert_eq!(cluster_domain, KUBERNETES_CLUSTER_DOMAIN_DEFAULT);
        }
    }

    #[test]
    fn parse_resolv_conf_error_no_search_entry() {
        let lines = read_file_from_string(KUBERNETES_RESOLV_CONF_MISSING_SEARCH_ENTRIES);
        let last_search_entry = find_last_search_entry(lines.as_slice());
        assert_eq!(last_search_entry, None);
    }
}
