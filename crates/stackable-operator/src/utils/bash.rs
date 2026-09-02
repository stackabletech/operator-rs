/// This is a bash snippet, which adds two functions out of interest:
///
/// 1. `prepare_signal_handlers` call this first to set up the needed traps
/// 2. `wait_for_termination` waits for the PID you passed as the first argument to terminate and
///    returns its exit status
///
/// An example use could be
/// ```text
/// {COMMON_BASH_TRAP_FUNCTIONS}
/// echo "Run before startup"
/// prepare_signal_handlers
/// {hadoop_home}/bin/hdfs {role} &
/// product_exit_code=0
/// wait_for_termination $! || product_exit_code=$?
/// echo "Run after termination"
/// exit "${product_exit_code}"
/// ```
// A `wait` status above 128 can mean two things:
// 1. bash reports a signal death as `128 + signal`
// 2. an in-progress `wait` abort with such a status when a trapped signal arrives
//
// We wait a second time to get the actual child status, which immediately returns the
// status again in case 1. In case 2 the child is still running and we do need to wait.
//
// See https://www.gnu.org/software/bash/manual/html_node/Signals.html
pub const COMMON_BASH_TRAP_FUNCTIONS: &str = r#"
prepare_signal_handlers()
{
    unset term_child_pid
    unset term_kill_needed
    trap 'handle_term_signal' TERM
}

handle_term_signal()
{
    if [ -n "${term_child_pid:-}" ]; then
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
    term_child_status=$?
    trap - TERM
    if [ "${term_child_status}" -gt 128 ]; then
        wait ${term_child_pid} 2>/dev/null
        term_child_status=$?
    fi
    set -e
    return ${term_child_status}
}
"#;

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn container_command_exit_code(script: &str) -> i32 {
        Command::new("/bin/bash")
            .args(["-euo", "pipefail", "-c", script])
            .status()
            .expect("bash can be executed in the test environment")
            .code()
            .expect("the shell terminated regularly and not by a signal")
    }

    #[test]
    fn crash_of_child_process() {
        let exit_code = container_command_exit_code(&format!(
            "{COMMON_BASH_TRAP_FUNCTIONS}
prepare_signal_handlers
bash -c 'exit 42' &
wait_for_termination $!"
        ));

        assert_eq!(42, exit_code);
    }

    #[test]
    fn graceful_shutdown_of_child_process() {
        let exit_code = container_command_exit_code(&format!(
            "{COMMON_BASH_TRAP_FUNCTIONS}
prepare_signal_handlers
bash -c 'trap \"exit 7\" TERM; sleep 10 & wait $!' &
child_pid=$!
(sleep 0.2; kill -TERM $$) &
wait_for_termination $child_pid"
        ));

        assert_eq!(7, exit_code);
    }
}
