use serde::Serialize;
use sysinfo::System;

use crate::error::ok_or_warn;

#[derive(Debug, Serialize)]
pub struct OperatingSystem {
    pub name: Option<String>,
    pub kernel_version: Option<String>,
    pub version: Option<String>,
    pub host_name: Option<String>,
    pub cpu_arch: String,
}

impl OperatingSystem {
    #[tracing::instrument(name = "OperatingSystem::collect")]
    pub fn collect() -> Self {
        let os = Self {
            name: ok_or_warn("os name", System::name()),
            kernel_version: ok_or_warn("kernel version", System::kernel_version()),
            version: ok_or_warn("os version", System::long_os_version()),
            host_name: ok_or_warn("host name", System::host_name()),
            cpu_arch: System::cpu_arch(),
        };
        tracing::info!(
            os.name,
            os.kernel.version = os.kernel_version,
            os.version,
            os.host_name,
            os.cpu_arch,
            "operating system",
        );
        os
    }
}
