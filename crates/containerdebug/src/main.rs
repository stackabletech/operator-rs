mod error;
mod system_information;

use clap::{crate_description, crate_version, Parser};
use stackable_operator::logging::TracingTarget;
use std::path::PathBuf;

use crate::system_information::SystemInformation;
use std::time::Instant;

const APP_NAME: &str = "containerdebug";

/// Collects and prints helpful debugging information about the environment that it is running in.
#[derive(clap::Parser)]
struct Opts {
    /// Loop every DURATION, instead of shutting down once completed (default DURATION: 1m)
    #[clap(
        long = "loop",
        value_name = "INTERVAL",
        default_missing_value = "1m",
        num_args = 0..=1,
        require_equals = true,
    )]
    loop_interval: Option<stackable_operator::time::Duration>,

    #[clap(long, short = 'o')]
    output: Option<PathBuf>,

    /// Tracing log collector system
    #[arg(long, env, default_value_t, value_enum)]
    pub tracing_target: TracingTarget,
}

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() {
    let opts = Opts::parse();
    stackable_operator::logging::initialize_logging(
        "CONTAINERDEBUG_LOG",
        APP_NAME,
        opts.tracing_target,
    );

    // Wrap *all* output in a span, to separate it from main app output.
    let _span = tracing::error_span!("containerdebug").entered();

    stackable_operator::utils::print_startup_string(
        crate_description!(),
        crate_version!(),
        built_info::GIT_VERSION,
        built_info::TARGET,
        built_info::BUILT_TIME_UTC,
        built_info::RUSTC_VERSION,
    );

    let mut next_run = Instant::now();
    loop {
        let next_run_sleep = next_run.saturating_duration_since(Instant::now());
        if !next_run_sleep.is_zero() {
            tracing::info!(?next_run, "scheduling next run...");
        }
        std::thread::sleep(next_run_sleep);

        let system_information = SystemInformation::collect();

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
