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
