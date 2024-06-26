use std::{
    io::{sink, Sink},
    path::PathBuf,
};

use tracing;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::{
    fmt::{
        writer::{EitherWriter, MakeWriterExt as _},
        MakeWriter,
    },
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

pub mod controller;
mod k8s_events;

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum TracingTarget {
    None,
    Jaeger,
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
pub fn initialize_logging(env: &str, app_name: &str, tracing_target: TracingTarget) {
    let filter = match EnvFilter::try_from_env(env) {
        Ok(env_filter) => env_filter,
        _ => EnvFilter::try_new(tracing::Level::INFO.to_string())
            .expect("Failed to initialize default tracing level to INFO"),
    };

    let file_appender_directory = std::env::var_os(format!("{env}_DIRECTORY")).map(PathBuf::from);
    let file_appender =
        OptionalMakeWriter::from(file_appender_directory.as_deref().map(|log_dir| {
            RollingFileAppender::builder()
                .filename_suffix(format!("{app_name}.log"))
                .max_log_files(6)
                .build(log_dir)
                .expect("failed to initialize rolling file appender")
        }));

    let fmt = tracing_subscriber::fmt::layer().with_writer(std::io::stdout.and(file_appender));
    let registry = Registry::default().with(filter).with(fmt);

    match tracing_target {
        TracingTarget::None => registry.init(),
        TracingTarget::Jaeger => {
            // FIXME (@Techassi): Replace with opentelemetry_otlp
            #[allow(deprecated)]
            let jaeger = opentelemetry_jaeger::new_agent_pipeline()
                .with_service_name(app_name)
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .expect("Failed to initialize Jaeger pipeline");
            let opentelemetry = tracing_opentelemetry::layer().with_tracer(jaeger);
            registry.with(opentelemetry).init();
        }
    }

    // need to delay logging until after tracing is initialized
    match file_appender_directory {
        Some(dir) => tracing::info!(directory = %dir.display(), "file logging enabled"),
        None => tracing::debug!("file logging disabled, because no log directory set"),
    }
}

/// Like [`EitherWriter`] but implements [`MakeWriter`] instead of [`std::io::Write`].
/// For selecting writers depending on dynamic configuration.
enum EitherMakeWriter<A, B> {
    A(A),
    B(B),
}
impl<'a, A, B> MakeWriter<'a> for EitherMakeWriter<A, B>
where
    A: MakeWriter<'a>,
    B: MakeWriter<'a>,
{
    type Writer = EitherWriter<A::Writer, B::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        match self {
            Self::A(a) => EitherWriter::A(a.make_writer()),
            Self::B(b) => EitherWriter::B(b.make_writer()),
        }
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        match self {
            Self::A(a) => EitherWriter::A(a.make_writer_for(meta)),
            Self::B(b) => EitherWriter::B(b.make_writer_for(meta)),
        }
    }
}

type OptionalMakeWriter<T> = EitherMakeWriter<T, fn() -> Sink>;
impl<T> From<Option<T>> for OptionalMakeWriter<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(t) => Self::A(t),
            None => Self::B(sink),
        }
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
