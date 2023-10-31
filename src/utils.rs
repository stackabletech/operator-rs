use tracing::info;

/// This is a bash snippet, which adds two functions out of interest:
///
/// 1. `prepare_signal_handlers` call this first to set up the needed traps
/// 2. `wait_for_termination` waits for the PID you passed as the first argument to terminate
///
/// An example use could be
/// ```text
/// {COMMON_BASH_TRAP_FUNCTIONS}
/// echo "Run before startup"
/// prepare_signal_handlers
/// {hadoop_home}/bin/hdfs {role} &
/// wait_for_termination $!
/// echo "Run after termination"
/// ```
pub const COMMON_BASH_TRAP_FUNCTIONS: &str = r#"
prepare_signal_handlers()
{
    unset term_child_pid
    unset term_kill_needed
    trap 'handle_term_signal' TERM
}

handle_term_signal()
{
    if [ "${term_child_pid}" ]; then
        kill -TERM "${term_child_pid}" 2>/dev/null
    else
        term_kill_needed="yes"
    fi
}

wait_for_termination()
{
    set +e
    term_child_pid=$1
    if [[ -v term_kill_needed ]]; then
        kill -TERM "${term_child_pid}" 2>/dev/null
    fi
    wait ${term_child_pid} 2>/dev/null
    trap - TERM
    wait ${term_child_pid} 2>/dev/null
    set -e
}
"#;

/// Prints helpful and standardized diagnostic messages.
///
/// This method is meant to be called first thing in the `main` method of an Operator.
///
/// # Usage
///
/// Use the [`built`](https://crates.io/crates/built) crate and include it in your `main.rs` like this:
///
/// ```text
/// mod built_info {
///     // The file has been placed there by the build script.
///     include!(concat!(env!("OUT_DIR"), "/built.rs"));
/// }
/// ```
///
/// Then call this method in your `main` method:
///
/// ```text
/// stackable_operator::utils::print_startup_string(
///      built_info::PKG_DESCRIPTION,
///      built_info::PKG_VERSION,
///      built_info::GIT_VERSION,
///      built_info::TARGET,
///      built_info::BUILT_TIME_UTC,
///      built_info::RUSTC_VERSION,
/// );
/// ```
pub fn print_startup_string(
    pkg_description: &str,
    pkg_version: &str,
    git_version: Option<&str>,
    target: &str,
    built_time: &str,
    rustc_version: &str,
) {
    let git_information = match git_version {
        None => "".to_string(),
        Some(git) => format!(" (Git information: {git})"),
    };
    info!("Starting {}", pkg_description);
    info!(
        "This is version {}{}, built for {} by {} at {}",
        pkg_version, git_information, target, rustc_version, built_time
    )
}

/// Returns the fully qualified controller name, which should be used when a single controller needs to be referred to uniquely.
///
/// `operator` should be a FQDN-style operator name (for example: `zookeeper.stackable.tech`).
/// `controller` should typically be the lower-case version of the primary resource that the
/// controller manages (for example: `zookeepercluster`).
pub(crate) fn format_full_controller_name(operator: &str, controller: &str) -> String {
    format!("{operator}_{controller}")
}
