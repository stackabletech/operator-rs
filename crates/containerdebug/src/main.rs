mod error;
mod system_information;

use clap::Parser;
use stackable_operator::telemetry::Tracing;
use std::path::PathBuf;

use crate::system_information::SystemInformation;
use stackable_operator::telemetry::tracing::TelemetryOptions;
use std::time::Instant;

const APP_NAME: &str = "containerdebug";

/// Collects and prints helpful debugging information about the environment that it is running in.
#[derive(clap::Parser)]
struct Opts {
    /// Loop every DURATION, instead of shutting down once completed (default DURATION: 30m)
    #[clap(
        long = "loop",
        value_name = "INTERVAL",
        default_missing_value = "30m",
        num_args = 0..=1,
        require_equals = true,
    )]
    loop_interval: Option<stackable_operator::time::Duration>,

    /// Write collected information to OUTPUT as JSON
    #[clap(long, short = 'o')]
    output: Option<PathBuf>,

    #[clap(flatten)]
    pub telemetry_arguments: TelemetryOptions,
}

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() {
    let opts = Opts::parse();

    let _trace_guard = Tracing::pre_configured(APP_NAME, opts.telemetry_arguments)
        .init()
        .unwrap();

    let init_span = tracing::error_span!("containerdebug init").entered();

    tracing::info!(
        built_info.pkg_version = built_info::PKG_VERSION,
        built_info.git_version = built_info::GIT_VERSION,
        built_info.target = built_info::TARGET,
        built_info.built_time_utc = built_info::BUILT_TIME_UTC,
        built_info.rustc_version = built_info::RUSTC_VERSION,
        "Starting {name}",
        name = built_info::PKG_NAME
    );

    let mut collect_ctx = SystemInformation::init();

    let mut next_run = Instant::now();

    drop(init_span);
    loop {
        // Wrap *all* output in a span, to separate it from main app output.
        let _span = tracing::error_span!("containerdebug run").entered();

        let next_run_sleep = next_run.saturating_duration_since(Instant::now());
        if !next_run_sleep.is_zero() {
            tracing::info!(?next_run, "scheduling next run...");
        }
        std::thread::sleep(next_run_sleep);

        let system_information = SystemInformation::collect(&mut collect_ctx);

        let serialized = serde_json::to_string_pretty(&system_information).unwrap();
        if let Some(output_path) = &opts.output {
            std::fs::write(output_path, &serialized).unwrap();
        }

        match opts.loop_interval {
            Some(interval) => next_run += interval,
            None => break,
        }
    }
}
