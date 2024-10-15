use serde::Serialize;
use std::collections::HashMap;
use std::net::IpAddr;

#[derive(Debug, Serialize)]
pub struct SystemInformation {
    pub cpu_count: usize,
    pub physical_core_count: Option<usize>,

    pub total_memory: u64,
    pub free_memory: u64,
    pub available_memory: u64,
    pub used_memory: u64,

    pub total_swap: u64,
    pub free_swap: u64,
    pub used_swap: u64,

    pub total_memory_cgroup: Option<u64>,
    pub free_memory_cgroup: Option<u64>,
    pub free_swap_cgroup: Option<u64>,

    pub system_name: Option<String>,
    pub kernel_version: Option<String>,
    pub os_version: Option<String>,
    pub host_name: Option<String>,
    pub cpu_arch: Option<String>,

    pub current_user: String, // The name of the current user
    pub current_uid: u32,     // The user ID (UID)
    pub current_gid: u32,     // The group ID (GID)

    pub disks: Vec<Disk>,
    pub network_information: SystemNetworkInfo,
}

#[derive(Debug, Serialize)]
pub struct Disk {
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
}

impl From<&sysinfo::Disk> for Disk {
    fn from(disk: &sysinfo::Disk) -> Self {
        Disk {
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
        }
    }
}

/// Captures all system network information, including network interfaces,
/// and the results of reverse and forward DNS lookups.
#[derive(Debug, Serialize)]
pub struct SystemNetworkInfo {
    pub network_interfaces: HashMap<String, Vec<IpAddr>>,
    pub reverse_lookups: HashMap<IpAddr, Vec<String>>,
    pub forward_lookups: HashMap<String, Vec<IpAddr>>,
}
