use tracing::info;

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
        Some(git) => format!(" (Git information: {})", git),
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
/// `controller` should typically be the lower-case plural version of the primary resource that the
/// controller manages (for example: `zookeeperclusters`).
pub(crate) fn format_full_controller_name(operator: &str, controller: &str) -> String {
    format!("{operator}/{controller}")
}
