/// This is a bash snippet, which adds two functions out of interest:
///
/// 1. `prepare_signal_handlers` call this first to set up the needed traps
/// 2. `wait_for_termination` waits for the PID you passed as the first argument to terminate and
///    returns its exit status
///
/// A SIGTERM which arrives after `prepare_signal_handlers` but before `wait_for_termination` is
/// called exits the script with status `143`. A product already started in that window is killed
/// with the container instead of shut down gracefully.
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
// 2. an in-progress `wait` aborts with such a status when a trapped signal arrives
//
// We wait a second time to get the actual child status, which immediately returns the
// status again in case 1. In case 2 the child is still running and we do need to wait.
//
// See https://www.gnu.org/software/bash/manual/html_node/Signals.html
//
// A SIGTERM before the child PID is known exits the script instead of being forwarded once the
// child is started: bash 5.1 (UBI 9) regularly loses a SIGTERM sent to a child right after it
// was forked, and the child then runs until the termination grace period expires.
pub const COMMON_BASH_TRAP_FUNCTIONS: &str = r#"
prepare_signal_handlers()
{
    unset term_child_pid
    trap 'handle_term_signal' TERM
}

handle_term_signal()
{
    if [ -n "${term_child_pid:-}" ]; then
        kill -TERM "${term_child_pid}" 2>/dev/null
    else
        exit 143
    fi
}

wait_for_termination()
{
    set +e
    term_child_pid=$1
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

/// Runs the given script with the shell options of a product container and returns its exit
/// status and stdout.
#[cfg(test)]
pub(crate) fn run_container_script(script: &str) -> (i32, String) {
    let output = std::process::Command::new("/bin/bash")
        .args(["-euo", "pipefail", "-c", script])
        .output()
        .expect("bash should be executable in the test environment");
    let exit_code = output
        .status
        .code()
        .expect("the script should terminate regularly and not by a signal");
    let stdout = String::from_utf8(output.stdout).expect("the script should only print UTF-8");

    (exit_code, stdout)
}

#[cfg(test)]
mod tests {
    use indoc::formatdoc;

    use super::*;

    #[test]
    fn crash_with_command_after_the_call() {
        let (exit_code, stdout) = run_container_script(&formatdoc! {"
            {COMMON_BASH_TRAP_FUNCTIONS}
            prepare_signal_handlers
            bash -c 'exit 42' &
            product_exit_code=0
            wait_for_termination $! || product_exit_code=$?
            echo tail-ran
            exit \"${{product_exit_code}}\""
        });

        assert_eq!(42, exit_code);
        assert!(stdout.contains("tail-ran\n"));
    }

    #[test]
    fn sigterm_before_child_pid_is_known() {
        let (exit_code, stdout) = run_container_script(&formatdoc! {"
            {COMMON_BASH_TRAP_FUNCTIONS}
            prepare_signal_handlers
            kill -TERM $$
            echo child-started
            sleep 10 &
            wait_for_termination $!"
        });

        assert_eq!(143, exit_code);
        assert!(!stdout.contains("child-started"));
    }
}
