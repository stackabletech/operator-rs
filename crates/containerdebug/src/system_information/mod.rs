use serde::Serialize;

use crate::error::ComponentResult;

pub mod disk;
pub mod network;
pub mod os;
pub mod resources;
pub mod user;

#[derive(Debug, Serialize)]
pub struct SystemInformation {
    pub resources: resources::Resources,
    pub os: os::OperatingSystem,
    pub current_user: ComponentResult<user::User>,
    pub disks: Vec<disk::Disk>,
    pub network: network::SystemNetworkInfo,
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
}

impl SystemInformation {
    #[tracing::instrument(name = "SystemInformation::collect")]
    pub fn collect() -> Self {
        tracing::info!("Starting data collection");

        // Please note that we use "new_all" to ensure that all list of
        // components, network interfaces, disks and users are already
        // filled!
        let sys = sysinfo::System::new_all();

        let info = Self {
            resources: resources::Resources::collect(&sys),
            os: os::OperatingSystem::collect(),
            current_user: ComponentResult::report_from_result(
                "User::collect_current",
                user::User::collect_current(&sys),
            ),
            disks: disk::Disk::collect_all(),
            network: network::SystemNetworkInfo::collect(),
        };
        tracing::info!("Data collection finished");
        info
    }
}
