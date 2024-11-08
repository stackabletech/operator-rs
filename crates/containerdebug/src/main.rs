mod system_information;

use clap::Parser;
use local_ip_address::list_afinet_netifas;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use sysinfo::{Disks, System};

use crate::system_information::{SystemInformation, SystemNetworkInfo};
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::Resolver;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Collects and prints helpful debugging information about the environment that it is running in.
#[derive(clap::Parser)]
struct Opts {
    /// Loop every <DURATION>, instead of shutting down once completed (default <DURATION>: 1m)
    #[clap(
        long = "loop",
        value_name = "INTERVAL",
        default_missing_value = "1m",
        num_args = 0..=1,
        require_equals = true,
    )]
    loop_interval: Option<stackable_operator::time::Duration>,

    #[clap(long, short = 'o')]
    output: Option<PathBuf>,
}

fn main() {
    tracing_subscriber::fmt().init();

    let opts = Opts::parse();

    let mut next_run = Instant::now();
    loop {
        std::thread::sleep(next_run.saturating_duration_since(Instant::now()));

        tracing::info!("starting run");

        // Please note that we use "new_all" to ensure that all list of
        // components, network interfaces, disks and users are already
        // filled!
        let sys = System::new_all();

        let disks = Disks::new_with_refreshed_list();
        let disks = disks
            .into_iter()
            .map(system_information::Disk::from)
            .collect();

        let system_network_information = get_network_info();

        let current_uid = users::get_current_uid();
        let current_gid = users::get_current_gid();
        let current_user = users::get_user_by_uid(current_uid).unwrap();

        let system_information = SystemInformation {
            cpu_count: sys.cpus().len(),
            physical_core_count: sys.physical_core_count(),

            total_memory: sys.total_memory(),
            free_memory: sys.free_memory(),
            available_memory: sys.available_memory(),
            used_memory: sys.used_memory(),

            total_swap: sys.total_swap(),
            free_swap: sys.free_swap(),
            used_swap: sys.used_swap(),

            total_memory_cgroup: sys.cgroup_limits().map(|limit| limit.total_memory),
            free_memory_cgroup: sys.cgroup_limits().map(|limit| limit.free_memory),
            free_swap_cgroup: sys.cgroup_limits().map(|limit| limit.free_swap),

            system_name: System::name(),
            kernel_version: System::kernel_version(),
            os_version: System::long_os_version(),
            host_name: System::host_name(),
            cpu_arch: System::cpu_arch(),

            disks,

            network_information: system_network_information,

            // Adding current user, UID, and GID info
            current_user: current_user.name().to_str().unwrap().to_string(),
            current_uid,
            current_gid,
        };

        let serialized = serde_json::to_string_pretty(&system_information).unwrap();
        println!("{}", serialized);
        if let Some(output_path) = &opts.output {
            std::fs::write(output_path, &serialized).unwrap();
        }

        // TODO:
        //  Current time
        //  SElinux/AppArmor
        //  Maybe env variables (may contain secrets)
        //  dmesg/syslog?
        //  capabilities?
        //  downward API
        //  Somehow get the custom resources logged?

        // Things left out for now because it doesn't seem too useful:
        // - Running processes
        // - Uptime/boot time
        // - Load average
        // - Network utilization
        // - Users/Groups

        match opts.loop_interval {
            Some(interval) => {
                next_run += interval;
                tracing::info!(?next_run, "run completed, scheduling next...");
            }
            None => break,
        }
    }
}

fn get_network_info() -> SystemNetworkInfo {
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
                interface_map
                    .entry(name)
                    .or_insert_with(Vec::new)
                    .push(ip_addr);
            }
            interface_map
        }
        Err(_) => HashMap::new(),
    };

    let mut ip_set: HashSet<IpAddr> = HashSet::new();
    for (_, ip_addrs) in interfaces.iter() {
        for ip_addr in ip_addrs {
            ip_set.insert(*ip_addr);
        }
    }

    let mut reverse_lookups = HashMap::new();
    for ip in ip_set {
        if let Ok(result) = resolver.reverse_lookup(ip) {
            for ptr_record in result {
                let hostname = ptr_record.to_utf8();
                reverse_lookups
                    .entry(ip)
                    .or_insert_with(Vec::new)
                    .push(hostname);
            }
        }
    }

    let mut hostname_set: HashSet<String> = HashSet::new();
    for (_, hostnames) in reverse_lookups.iter() {
        for hostname in hostnames {
            hostname_set.insert(hostname.clone());
        }
    }

    let mut forward_lookups = HashMap::new();
    for hostname in hostname_set {
        if let Ok(result) = resolver.lookup_ip(hostname.clone()) {
            for ip_addr in result {
                forward_lookups
                    .entry(hostname.clone())
                    .or_insert_with(Vec::new)
                    .push(ip_addr);
            }
        }
    }

    SystemNetworkInfo {
        network_interfaces: interfaces,
        reverse_lookups,
        forward_lookups,
    }
}
