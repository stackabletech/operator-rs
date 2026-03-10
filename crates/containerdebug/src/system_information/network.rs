use hickory_resolver::{
    TokioResolver, name_server::TokioConnectionProvider, system_conf::read_system_conf,
};
use local_ip_address::list_afinet_netifas;
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    sync::LazyLock,
    time::Duration,
};
use tokio::task::JoinSet;

static GLOBAL_DNS_RESOLVER: LazyLock<TokioResolver> = LazyLock::new(|| {
    let (resolver_config, mut resolver_opts) =
        read_system_conf().expect("failed to read system resolv config");
    resolver_opts.timeout = Duration::from_secs(5);

    TokioResolver::builder_with_config(resolver_config, TokioConnectionProvider::default())
        .with_options(resolver_opts)
        .build()
});

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
    pub async fn collect() -> SystemNetworkInfo {
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

        let ips: BTreeSet<IpAddr> = interfaces.values().flatten().copied().collect();
        tracing::info!(network.addresses.ip = ?ips, "ip addresses");

        let mut reverse_lookups = JoinSet::new();
        for ip in ips {
            reverse_lookups
                .spawn(async move { (ip, GLOBAL_DNS_RESOLVER.reverse_lookup(ip).await) });
        }
        let reverse_lookups: HashMap<IpAddr, Vec<String>> = reverse_lookups
            .join_all()
            .await
            .into_iter()
            .filter_map(|(ip, reverse_lookup)| match reverse_lookup {
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

        let mut forward_lookups = JoinSet::new();
        for hostname in hostname_set {
            forward_lookups.spawn(async move {
                (
                    hostname.clone(),
                    GLOBAL_DNS_RESOLVER.lookup_ip(hostname).await,
                )
            });
        }
        let forward_lookups: HashMap<String, Vec<IpAddr>> = forward_lookups
            .join_all()
            .await
            .into_iter()
            .filter_map(|(hostname, forward_lookup)| match forward_lookup {
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
