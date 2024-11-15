use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Disk {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
}

impl Disk {
    #[tracing::instrument(name = "Disk::collect_all")]
    pub fn collect_all() -> Vec<Self> {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        if disks.into_iter().next().is_none() {
            tracing::info!("no disks found");
        }
        disks.into_iter().map(Self::from).collect()
    }
}

impl From<&sysinfo::Disk> for Disk {
    fn from(sysinfo_disk: &sysinfo::Disk) -> Self {
        let disk = Disk {
            name: sysinfo_disk.name().to_string_lossy().into_owned(),
            mount_point: sysinfo_disk.mount_point().to_string_lossy().into_owned(),
            total_space: sysinfo_disk.total_space(),
            available_space: sysinfo_disk.available_space(),
        };
        tracing::info!(
            disk.mount_point,
            disk.name,
            disk.space.total = disk.total_space,
            disk.space.available = disk.available_space,
            "found disk"
        );
        disk
    }
}
