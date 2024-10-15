use snafu::{ResultExt, Snafu};
use std::{
    env,
    io::{self, BufRead},
    path::Path,
    sync::LazyLock,
};

use crate::commons::networking::DomainName;

const KUBERNETES_SERVICE_DNS_DOMAIN: &str = "KUBERNETES_SERVICE_DNS_DOMAIN";
const KUBERNETES_SERVICE_DNS_DOMAIN_DEFAULT: &str = "cluster.local";

const KUBERNETES_SERVICE_HOST: &str = "KUBERNETES_SERVICE_HOST";

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

    #[snafu(display("Could not find any 'search' entry in '{resolve_conf_file_path}'."))]
    SearchEntryNotFound { resolve_conf_file_path: String },
}

/// This is the primary entry point to retrieve the Kubernetes service DNS domain.
///
/// Implements the logic decided in <https://github.com/stackabletech/issues/issues/436>
///
/// 1. Check if KUBERNETES_SERVICE_DNS_DOMAIN is set -> return if set
/// 2. Check if KUBERNETES_SERVICE_HOST is set to determine if we run in a Kubernetes / Openshift cluster
/// 2.1 If set continue and parse the `resolv.conf`
/// 2.2 If not set default to `cluster.local`
/// 3. Read and parse the `resolv.conf`.
///
/// NOTE: 
/// The whole code has many many unwraps and expects.
/// Since this code will be evaluated once and is crucial for
/// successful cluster deployments, we actually want to *crash*
/// the operator if we cannot find a proper value for the service dns domain.
///
/// # Usage
///
/// ```
/// let kubernetes_service_dns_domain = *SERVICE_DNS_DOMAIN;
/// ```
///
pub static SERVICE_DNS_DOMAIN: LazyLock<DomainName> = LazyLock::new(|| {
    // 1. Read KUBERNETES_SERVICE_DNS_DOMAIN env var
    tracing::info!("Trying to determine the Kubernetes service DNS domain...");
    match read_env_var(KUBERNETES_SERVICE_DNS_DOMAIN) {
        Ok(service_dns_domain) => return service_dns_domain.try_into().unwrap(),
        Err(_) => {
            tracing::info!("The env var '{KUBERNETES_SERVICE_DNS_DOMAIN}' is not set.");
        }
    };

    // 2. If no env var is set, check if we run in a clusterized (Kubernetes/Openshift) enviroment
    //    by checking if KUBERNETES_SERVICE_HOST is set: If not default to 'cluster.local'.
    tracing::info!("Trying to determine the runtime environment...");
    if read_env_var(KUBERNETES_SERVICE_HOST).is_err() {
        tracing::info!("The env var '{KUBERNETES_SERVICE_HOST}' is not set. This means we do not run in Kubernetes / Openshift. Defaulting DNS domain to '{KUBERNETES_SERVICE_DNS_DOMAIN_DEFAULT}'.");
        // The unwrap is safe here and should never fail
        return KUBERNETES_SERVICE_DNS_DOMAIN_DEFAULT
            .to_string()
            .try_into()
            .unwrap();
    }

    // 3. Read and parse 'resolv.conf'. We are looking for the last "search" entry and filter for the shortest
    //    element in that search line
    let resolve_conf_lines = read_file_from_path(RESOLVE_CONF_FILE_PATH)
        .context(ResolvConfNotFoundSnafu {
            resolve_conf_file_path: RESOLVE_CONF_FILE_PATH.to_string(),
        }).unwrap();

    parse_resolve_config(resolve_conf_lines).try_into().unwrap()
});

fn parse_resolve_config(resolv_conf: Vec<String>) -> String {
    tracing::debug!(
        "Start parsing '{RESOLVE_CONF_FILE_PATH}' to retrieve the Kubernetes service DNS domain..."
    );

    // The unwraps/expects here are to hard abort if this fails.
    // This will crash the operator at the start which is desired in that case.
    let last_search_entry = find_last_search_entry(&resolv_conf)
        .expect("No 'search' entries found in '{RESOLVE_CONF_FILE_PATH}'. Aborting...");
    let last_search_entry_content = parse_search_line(&last_search_entry)
        .expect("No 'search' entry found in {last_search_entry}. Aborting...");
    let shortest_search_entry = find_shortest_entry(last_search_entry_content)
        .expect("No valid entries found in the line '{last_search_entry_content}'. Aborting...");

    shortest_search_entry.to_string()
}

fn read_env_var(name: &str) -> Result<String, Error> {
    env::var(name).context(EnvVarNotFoundSnafu { name })
}

// Function to read the contents of a file and return all lines as Vec<String>
fn read_file_from_path(resolv_conf_file_path: &str) -> Result<Vec<String>, io::Error> {
    let file = std::fs::File::open(Path::new(resolv_conf_file_path))?;
    let reader = io::BufReader::new(file);

    reader.lines().collect()
}

// Function to find the last search entry in the lines
fn find_last_search_entry(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .rev() // Start from the end to find the last occurrence
        .find(|line| line.trim().starts_with("search"))
        .cloned() // Convert the reference to a cloned String
}

// Function to remove the "search" keyword and return the remaining entries
fn parse_search_line(search_line: &str) -> Option<&str> {
    search_line.trim().strip_prefix("search")
}

// Function to find the shortest entry in the parsed search line
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

    fn read_file_from_string(contents: &str) -> Vec<String> {
        // Split the string by lines and collect into a Vec<String>
        contents.lines().map(|line| line.to_string()).collect()
    }

    #[test]
    fn use_different_kubernetes_service_dns_domain_value() {
        let service_dns_domain = "my-cluster.local".to_string();
        unsafe {
            env::set_var(KUBERNETES_SERVICE_DNS_DOMAIN, &service_dns_domain);
        }
        assert_eq!(*SERVICE_DNS_DOMAIN, service_dns_domain.try_into().unwrap());
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
            let search_entry = parse_search_line(&last_search_entry).unwrap();
            let service_dns_domain = find_shortest_entry(search_entry).unwrap();
            assert_eq!(service_dns_domain, "cluster.local")
        }
    }

    #[test]
    fn parse_resolv_conf_error_no_search_entry() {
        let lines = read_file_from_string(KUBERNETES_RESOLV_CONF_MISSING_SEARCH_ENTRIES);
        let last_search_entry = find_last_search_entry(lines.as_slice());
        assert_eq!(last_search_entry, None)
    }
}
