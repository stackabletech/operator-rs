use serde::Serialize;

use crate::error::ComponentResult;

pub mod disk;
pub mod network;
pub mod os;
pub mod resources;
pub mod user;

#[derive(Debug, Serialize, Default)]
pub struct SystemInformation {
    // All fields are optional, to make it easy to disable modules one by one
    pub resources: Option<resources::Resources>,
    pub os: Option<os::OperatingSystem>,
    pub current_user: Option<ComponentResult<user::User>>,
    pub disks: Option<Vec<disk::Disk>>,
    pub network: Option<network::SystemNetworkInfo>,
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

/// Common data that is cached between [`SystemInformation::collect`] calls.
pub struct CollectContext {
    system: sysinfo::System,
}

impl SystemInformation {
    /// Collects static information that doesn't need to be refreshed.
    #[tracing::instrument(name = "SystemInformation::init")]
    pub fn init() -> CollectContext {
        tracing::info!("initializing");
        let mut ctx = CollectContext {
            // Each module is responsible for updating the information that it cares about.
            system: sysinfo::System::new(),
        };
        if let Err(err) = user::User::init(&mut ctx.system) {
            tracing::error!(
                error = &err as &dyn std::error::Error,
                "failed to initialize user module, ignoring but this will likely cause collection errors..."
            );
        }
        tracing::info!("init finished");
        ctx
    }

    /// Collects and reports
    #[tracing::instrument(name = "SystemInformation::collect", skip(ctx))]
    pub fn collect(ctx: &mut CollectContext) -> Self {
        tracing::info!("Starting data collection");

        let info = Self {
            resources: Some(resources::Resources::collect(&mut ctx.system)),
            os: Some(os::OperatingSystem::collect()),
            current_user: Some(ComponentResult::report_from_result(
                "User::collect_current",
                user::User::collect_current(&ctx.system),
            )),
            disks: Some(disk::Disk::collect_all()),
            network: Some(network::SystemNetworkInfo::collect()),
            // ..Default::default()
        };

        tracing::info!("Data collection finished");
        info
    }
}
