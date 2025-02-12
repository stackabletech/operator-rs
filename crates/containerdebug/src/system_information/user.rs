use serde::Serialize;
use snafu::{OptionExt, ResultExt, Snafu};
use sysinfo::{Gid, Pid, ProcessRefreshKind, Uid, UpdateKind};

use crate::error::SysinfoError;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to get pid of the current process"))]
    GetCurrentPid { source: SysinfoError },
    #[snafu(display("current pid {pid} could not be resolved to a proess"))]
    ResolveCurrentProcess { pid: Pid },
}
type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Serialize)]
pub struct User {
    pub name: Option<String>, // The name of the current user
    pub uid: Option<Uid>,     // The user ID (UID)
    pub gid: Option<Gid>,     // The group ID (GID)
}

impl User {
    #[tracing::instrument(name = "User::init", skip(sys))]
    pub fn init(sys: &mut sysinfo::System) -> Result<()> {
        let pid = sysinfo::get_current_pid()
            .map_err(|msg| SysinfoError { msg })
            .context(GetCurrentPidSnafu)?;
        // The process user is static, and there is a memory leak to updating it for every run, so cache it once and keep that.
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing().with_user(UpdateKind::OnlyIfNotSet),
        );
        Ok(())
    }

    #[tracing::instrument(name = "User::collect_current", skip(sys))]
    pub fn collect_current(sys: &sysinfo::System) -> Result<Self> {
        let pid = sysinfo::get_current_pid()
            .map_err(|msg| SysinfoError { msg })
            .context(GetCurrentPidSnafu)?;
        let current_process = sys
            .process(pid)
            .context(ResolveCurrentProcessSnafu { pid })?;
        let uid = current_process.user_id();
        let os_users = sysinfo::Users::new_with_refreshed_list();
        let user = Self {
            name: uid.and_then(|uid| Some(os_users.get_user_by_id(uid)?.name().to_string())),
            uid: uid.cloned(),
            gid: current_process.group_id(),
        };
        tracing::info!(
            user.name,
            user.uid = user.uid.as_ref().map(|uid| format!("{uid:?}")),
            user.gid = user.uid.as_ref().map(|gid| format!("{gid:?}")),
            "current user"
        );
        Ok(user)
    }
}
