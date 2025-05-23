use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Serialize)]
pub struct Resources {
    pub cpu_count: usize,
    pub physical_core_count: Option<usize>,

    pub open_files_limit: Option<usize>,

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
}

impl Resources {
    #[tracing::instrument(name = "Resources::collect", skip(sys))]
    pub fn collect(sys: &mut System) -> Self {
        // This style of "declare-then-log-then-merge becomes a bit verbose,
        // but should help keep each log statement local to where that info is collected.

        sys.refresh_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::everything()),
        );

        let cpu_count = sys.cpus().len();
        let physical_core_count = System::physical_core_count();
        tracing::info!(
            cpus.physical = cpu_count,
            cpus.cores.physical = physical_core_count,
            "cpus"
        );

        let open_files_limit = System::open_files_limit();
        tracing::info!(open_files.limit = open_files_limit, "open files limit");

        let total_memory = sys.total_memory();
        let free_memory = sys.free_memory();
        let available_memory = sys.available_memory();
        let used_memory = sys.used_memory();
        tracing::info!(
            memory.total = total_memory,
            memory.free = free_memory,
            memory.available = available_memory,
            memory.used = used_memory,
            "memory"
        );

        let total_swap = sys.total_swap();
        let free_swap = sys.free_swap();
        let used_swap = sys.used_swap();
        tracing::info!(
            swap.total = total_swap,
            swap.free = free_swap,
            swap.used = used_swap,
            "swap"
        );

        let total_memory_cgroup;
        let free_memory_cgroup;
        let free_swap_cgroup;
        // FIXME: seems to be None even when running inside of a cgroup (via systemd-run --scope)? investigate
        if let Some(cgroup) = sys.cgroup_limits() {
            total_memory_cgroup = Some(cgroup.total_memory);
            free_memory_cgroup = Some(cgroup.free_memory);
            free_swap_cgroup = Some(cgroup.free_swap);
            tracing::info!(
                cgroup.memory.total = total_memory_cgroup,
                cgroup.memory.free = free_memory_cgroup,
                cgroup.swap.free = free_swap_cgroup,
                "cgroup memory"
            );
        } else {
            (total_memory_cgroup, free_memory_cgroup, free_swap_cgroup) = (None, None, None);
            tracing::info!("not in a cgroup")
        }

        Self {
            cpu_count,
            physical_core_count,

            open_files_limit,

            total_memory,
            free_memory,
            available_memory,
            used_memory,

            total_swap,
            free_swap,
            used_swap,

            total_memory_cgroup,
            free_memory_cgroup,
            free_swap_cgroup,
        }
    }
}
