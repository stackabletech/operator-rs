use hickory_resolver::{
    TokioResolver, name_server::TokioConnectionProvider, system_conf::read_system_conf,
};
use local_ip_address::list_afinet_netifas;
use serde::Serialize;
use snafu::{ResultExt, Snafu};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    sync::LazyLock,
    time::Duration,
};
use tokio::task::JoinSet;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to list network interfaces"))]
    ListInterfaces { source: local_ip_address::Error },
}

static GLOBAL_DNS_RESOLVER: LazyLock<Option<TokioResolver>> = LazyLock::new(|| {
    let (resolver_config, mut resolver_opts) = match read_system_conf() {
        Ok(conf) => conf,
        Err(err) => {
            tracing::error!(
                error = &err as &dyn std::error::Error,
                "failed to read system DNS config, DNS lookups will be skipped"
            );
            return None;
        }
    };
    resolver_opts.timeout = Duration::from_secs(5);

    Some(
        TokioResolver::builder_with_config(resolver_config, TokioConnectionProvider::default())
            .with_options(resolver_opts)
            .build(),
    )
});

/// Captures all system network information, including network interfaces,
/// and the results of reverse and forward DNS lookups.
#[derive(Debug, Serialize)]
pub struct SystemNetworkInfo {
    pub interfaces: BTreeMap<String, Vec<IpAddr>>,
    pub reverse_lookups: BTreeMap<IpAddr, Vec<String>>,
    pub forward_lookups: BTreeMap<String, Vec<IpAddr>>,
}

impl SystemNetworkInfo {
    #[tracing::instrument(name = "SystemNetworkInfo::collect")]
    pub async fn collect() -> Result<SystemNetworkInfo, Error> {
        let netifs = list_afinet_netifas().context(ListInterfacesSnafu)?;
        let mut interfaces = BTreeMap::new();

        // Iterate over the network interfaces and group them by name
        // An interface may appear multiple times if it has multiple IP addresses (e.g. IPv4 and IPv6)
        for (name, ip_addr) in netifs {
            tracing::info!(
                network.interface.name = name,
                network.interface.address = %ip_addr,
                "found network interface"
            );
            interfaces
                .entry(name)
                .or_insert_with(Vec::new)
                .push(ip_addr);
        }

        let ips: BTreeSet<IpAddr> = interfaces.values().flatten().copied().collect();
        tracing::info!(network.addresses.ip = ?ips, "ip addresses");

        let Some(resolver) = GLOBAL_DNS_RESOLVER.as_ref() else {
            return Ok(SystemNetworkInfo {
                interfaces,
                reverse_lookups: BTreeMap::new(),
                forward_lookups: BTreeMap::new(),
            });
        };

        let mut reverse_lookup_tasks = JoinSet::new();
        for ip in ips {
            reverse_lookup_tasks.spawn(async move { (ip, resolver.reverse_lookup(ip).await) });
        }
        let reverse_lookups: BTreeMap<IpAddr, Vec<String>> = reverse_lookup_tasks
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

        let mut forward_lookup_tasks = JoinSet::new();
        for hostname in hostname_set {
            forward_lookup_tasks
                .spawn(async move { (hostname.clone(), resolver.lookup_ip(hostname).await) });
        }
        let forward_lookups: BTreeMap<String, Vec<IpAddr>> = forward_lookup_tasks
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

        Ok(SystemNetworkInfo {
            interfaces,
            reverse_lookups,
            forward_lookups,
        })
    }
}
