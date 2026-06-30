use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Disk {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub used_space: u64,
    pub available_space: u64,
    pub usage_percent: f64,
}

impl Disk {
    #[tracing::instrument(name = "Disk::collect_all")]
    pub fn collect_all() -> Vec<Self> {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        if disks.list().is_empty() {
            tracing::info!("no disks found");
        }
        disks.list().iter().map(Self::from).collect()
    }
}

impl From<&sysinfo::Disk> for Disk {
    fn from(sysinfo_disk: &sysinfo::Disk) -> Self {
        let total_space = sysinfo_disk.total_space();
        let available_space = sysinfo_disk.available_space();
        // There shouldn't be negative used bytes. We prevent underflow, to not falsely report more
        // used than total space.
        let used_space = total_space.saturating_sub(available_space);
        let usage_percent = match used_space {
            0 => 0.0,
            used_space => used_space as f64 / total_space as f64 * 100.0,
        };

        let disk = Disk {
            name: sysinfo_disk.name().to_string_lossy().into_owned(),
            mount_point: sysinfo_disk.mount_point().to_string_lossy().into_owned(),
            total_space,
            used_space,
            available_space,
            usage_percent,
        };

        if usage_percent >= 85.0 {
            tracing::warn!(
                disk.mount_point,
                disk.name,
                disk.space.total = disk.total_space,
                disk.space.used = disk.used_space,
                disk.space.available = disk.available_space,
                disk.space.usage_percent = format!("{:.1}%", disk.usage_percent),
                "disk usage high"
            );
        } else {
            tracing::info!(
                disk.mount_point,
                disk.name,
                disk.space.total = disk.total_space,
                disk.space.used = disk.used_space,
                disk.space.available = disk.available_space,
                disk.space.usage_percent = format!("{:.1}%", disk.usage_percent),
                "found disk"
            );
        }
        disk
    }
}
