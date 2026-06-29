use serde::Serialize;
use sysinfo::System;

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
            name: System::name(),
            kernel_version: System::kernel_version(),
            version: System::long_os_version(),
            host_name: System::host_name(),
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
