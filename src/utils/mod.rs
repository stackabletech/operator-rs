pub mod bash;
pub mod logging;
mod url;

#[deprecated(
    note = "renamed to stackable_operator::utils::bash::COMMON_BASH_TRAP_FUNCTIONS",
    since = "0.61.1"
)]
pub use self::bash::COMMON_BASH_TRAP_FUNCTIONS;
#[deprecated(
    note = "renamed to stackable_operator::utils::logging::print_startup_string",
    since = "0.61.1"
)]
pub use self::logging::print_startup_string;
pub use self::url::UrlExt;

/// Returns the fully qualified controller name, which should be used when a single controller needs to be referred to uniquely.
///
/// `operator` should be a FQDN-style operator name (for example: `zookeeper.stackable.tech`).
/// `controller` should typically be the lower-case version of the primary resource that the
/// controller manages (for example: `zookeepercluster`).
pub(crate) fn format_full_controller_name(operator: &str, controller: &str) -> String {
    format!("{operator}_{controller}")
}
