use std::path::PathBuf;

use tracing;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub mod controller;
mod k8s_events;

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum TracingTarget {
    None,
}

impl Default for TracingTarget {
    fn default() -> Self {
        Self::None
    }
}

/// Initializes `tracing` logging with options from the environment variable
/// given in the `env` parameter.
///
/// We force users to provide a variable name so it can be different per product.
/// We encourage it to be the product name plus `_LOG`, e.g. `FOOBAR_OPERATOR_LOG`.
/// If no environment variable is provided, the maximum log level is set to INFO.
///
/// Log output can be copied to a file by setting `{env}_DIRECTORY` (e.g. `FOOBAR_OPERATOR_DIRECTORY`)
/// to a directory path. This file will be rotated regularly.
pub fn initialize_logging(env: &str, app_name: &str, _tracing_target: TracingTarget) {
    let filter = match EnvFilter::try_from_env(env) {
        Ok(env_filter) => env_filter,
        _ => EnvFilter::try_new(tracing::Level::INFO.to_string())
            .expect("Failed to initialize default tracing level to INFO"),
    };

    let terminal_fmt = tracing_subscriber::fmt::layer();

    let file_appender_directory = std::env::var_os(format!("{env}_DIRECTORY")).map(PathBuf::from);
    let file_fmt = file_appender_directory.as_deref().map(|log_dir| {
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::HOURLY)
            .filename_prefix(app_name.to_string())
            .filename_suffix("tracing-rs.json")
            .max_log_files(6)
            .build(log_dir)
            .expect("failed to initialize rolling file appender");
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(file_appender)
    });

    Registry::default()
        .with(filter)
        .with(terminal_fmt)
        .with(file_fmt)
        .init();

    // need to delay logging until after tracing is initialized
    match file_appender_directory {
        Some(dir) => tracing::info!(directory = %dir.display(), "file logging enabled"),
        None => tracing::debug!("file logging disabled, because no log directory set"),
    }
}

#[cfg(test)]
mod test {

    use tracing::{debug, error, info};

    use crate::logging::TracingTarget;

    // If there is a proper way to programmatically inspect the global max level than we should use that.
    // Until then, this is mostly a sanity check for the implementation above.
    // Either run
    //      cargo test default_tracing -- --nocapture
    // to see the ERROR and INFO messages, or
    //      NOT_SET=debug cargo test default_tracing -- --nocapture
    // to see them all.
    #[test]
    pub fn test_default_tracing_level_is_set_to_info() {
        super::initialize_logging("NOT_SET", "test", TracingTarget::None);

        error!("ERROR level messages should be seen.");
        info!("INFO level messages should also be seen by default.");
        debug!("DEBUG level messages should be seen only if you set the NOT_SET env var.");
    }
}
