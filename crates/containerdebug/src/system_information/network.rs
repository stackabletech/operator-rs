use hickory_resolver::{Resolver, system_conf::read_system_conf};
use local_ip_address::list_afinet_netifas;
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    time::Duration,
};

/// Captures all system network information, including network interfaces,
/// and the results of reverse and forward DNS lookups.
#[derive(Debug, Serialize)]
pub struct SystemNetworkInfo {
    pub interfaces: HashMap<String, Vec<IpAddr>>,
    pub reverse_lookups: HashMap<IpAddr, Vec<String>>,
    pub forward_lookups: HashMap<String, Vec<IpAddr>>,
}

impl SystemNetworkInfo {
    #[tracing::instrument(name = "SystemNetworkInfo::collect")]
    pub fn collect() -> SystemNetworkInfo {
        /*
        let resolver = Resolver::from_system_conf()
            .map_err(|e| e.to_string())
            .unwrap();
         */
        let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
        resolver_opts.timeout = Duration::from_secs(5);
        let resolver = Resolver::new(resolver_config, resolver_opts).unwrap();

        let interfaces = match list_afinet_netifas() {
            Ok(netifs) => {
                let mut interface_map = std::collections::HashMap::new();

                // Iterate over the network interfaces and group them by name
                // An interface may appear multiple times if it has multiple IP addresses (e.g. IPv4 and IPv6)
                for (name, ip_addr) in netifs {
                    tracing::info!(
                        network.interface.name = name,
                        network.interface.address = %ip_addr,
                        "found network interface"
                    );
                    interface_map
                        .entry(name)
                        .or_insert_with(Vec::new)
                        .push(ip_addr);
                }
                interface_map
            }
            Err(error) => {
                tracing::error!(
                    error = &error as &dyn std::error::Error,
                    "failed to list network interfaces"
                );
                HashMap::new()
            }
        };

        let ip_set: BTreeSet<IpAddr> = interfaces.values().flatten().copied().collect();
        tracing::info!(network.addresses.ip = ?ip_set, "ip addresses");

        let reverse_lookups: HashMap<IpAddr, Vec<String>> = ip_set
            .into_iter()
            .filter_map(|ip| match resolver.reverse_lookup(ip) {
                Ok(result) => {
                    let hostnames = result
                        .into_iter()
                        .map(|ptr_record| ptr_record.to_utf8())
                        .collect();
                    tracing::info!(%ip, ?hostnames, "performed reverse DNS lookup for IP");
                    Some((ip, hostnames))
                }
                Err(error) => {
                    tracing::warn!(
                        %ip,
                        error = &error as &dyn std::error::Error,
                        "reverse DNS lookup failed"
                    );
                    None
                }
            })
            .collect();

        let hostname_set: BTreeSet<String> = reverse_lookups.values().flatten().cloned().collect();
        tracing::info!(network.addresses.hostname = ?hostname_set, "hostnames");

        let forward_lookups: HashMap<String, Vec<IpAddr>> = hostname_set
            .into_iter()
            .filter_map(|hostname| match resolver.lookup_ip(hostname.clone()) {
                Ok(result) => {
                    let ips = result.iter().collect();
                    tracing::info!(hostname, ?ips, "performed forward DNS lookup for hostname");
                    Some((hostname, ips))
                }
                Err(error) => {
                    tracing::warn!(
                        hostname,
                        error = &error as &dyn std::error::Error,
                        "forward DNS lookup failed"
                    );
                    None
                }
            })
            .collect();

        SystemNetworkInfo {
            interfaces,
            reverse_lookups,
            forward_lookups,
        }
    }
}
