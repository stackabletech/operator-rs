//! This module contains functionality to initialise tracing Subscribers for
//! console output, file output, and OpenTelemetry OTLP export for traces and logs.
//!
//! It is intended to be used by the Stackable Data Platform operators and
//! webhooks, but it should be generic enough to be used in any application.
//!
//! To get started, see [`Tracing`].

use std::path::PathBuf;

use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use snafu::{ResultExt as _, Snafu};
use tracing::{level_filters::LevelFilter, subscriber::SetGlobalDefaultError};
use tracing_appender::rolling::{InitError, RollingFileAppender};
use tracing_subscriber::{EnvFilter, Layer, Registry, filter::Directive, layer::SubscriberExt};

use crate::tracing::settings::*;

pub mod settings;

type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors which can be encountered when initialising [`Tracing`].
#[derive(Debug, Snafu)]
pub enum Error {
    /// Indicates that [`Tracing`] failed to install the OpenTelemetry trace exporter.
    #[snafu(display("unable to install opentelemetry trace exporter"))]
    InstallOtelTraceExporter {
        #[allow(missing_docs)]
        source: opentelemetry::trace::TraceError,
    },

    /// Indicates that [`Tracing`] failed to install the OpenTelemetry log exporter.
    #[snafu(display("unable to install opentelemetry log exporter"))]
    InstallOtelLogExporter {
        #[allow(missing_docs)]
        source: opentelemetry_sdk::logs::LogError,
    },

    /// Indicates that [`Tracing`] failed to install the rolling file appender.
    #[snafu(display("failed to initialize rolling file appender"))]
    InitRollingFileAppender {
        #[allow(missing_docs)]
        source: InitError,
    },

    /// Indicates that [`Tracing`] failed to set the global default subscriber.
    #[snafu(display("unable to set the global default subscriber"))]
    SetGlobalDefaultSubscriber {
        #[allow(missing_docs)]
        source: SetGlobalDefaultError,
    },
}

/// Easily initialize a set of pre-configured [`Subscriber`][1] layers.
///
/// # Usage
///
/// There are two different styles to configure individual subscribers: Using the sophisticated
/// [`SettingsBuilder`] or the simplified tuple style for basic configuration. Currently, three
/// different subscribers are supported: console output, OTLP log export, and OTLP trace export.
///
/// The subscribers are active as long as the tracing guard returned by [`Tracing::init`] is in
/// scope and not dropped. Dropping it results in subscribers being shut down, which can lead to
/// loss of telemetry data when done before exiting the application. This is why it is important
/// to hold onto the guard as long as required.
///
/// <div class="warning">
/// Name the guard variable appropriately, do not just use <code>let _ =</code>, as that will drop
/// immediately.
/// </div>
///
/// ```
/// # use stackable_telemetry::tracing::{Tracing, Error};
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let _tracing_guard = Tracing::builder() // < Scope starts here
///         .service_name("test")               // |
///         .build()                            // |
///         .init()?;                           // |
///                                             // |
///     tracing::info!("log a message");        // |
///     Ok(())                                  // < Scope ends here, guard is dropped
/// }
/// ```
///
/// ## Basic configuration
///
/// A basic configuration of subscribers can be done by using 2-tuples or 3-tuples, also called
/// doubles and triples. Using tuples, the subscriber can be enabled/disabled and it's environment
/// variable and default level can be set.
///
/// ```
/// use stackable_telemetry::tracing::{Tracing, Error, settings::Settings};
/// use tracing_subscriber::filter::LevelFilter;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     // This can come from a Clap argument for example. The enabled builder
///     // function below allows enabling/disabling certain subscribers during
///     // runtime.
///     let otlp_log_flag = false;
///
///     let _tracing_guard = Tracing::builder()
///         .service_name("test")
///         .with_console_output(("TEST_CONSOLE", LevelFilter::INFO))
///         .with_otlp_log_exporter(("TEST_OTLP_LOG", LevelFilter::DEBUG, otlp_log_flag))
///         .build()
///         .init()?;
///
///     tracing::info!("log a message");
///
///     Ok(())
/// }
/// ```
///
/// ## Advanced configuration
///
/// More advanced configurations can be done via the [`Settings::builder`] function. Each
/// subscriber provides specific settings based on a common set of options. These options can be
/// customized via the following methods:
///
/// - [`SettingsBuilder::console_log_settings_builder`]
/// - [`SettingsBuilder::otlp_log_settings_builder`]
/// - [`SettingsBuilder::otlp_trace_settings_builder`]
///
/// ```
/// # use stackable_telemetry::tracing::{Tracing, Error, settings::Settings};
/// # use tracing_subscriber::filter::LevelFilter;
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     // Control the otlp_log subscriber at runtime
///     let otlp_log_flag = false;
///
///     let _tracing_guard = Tracing::builder()
///         .service_name("test")
///         .with_console_output(
///             Settings::builder()
///                 .with_environment_variable("TEST_CONSOLE")
///                 .with_default_level(LevelFilter::INFO)
///                 .build()
///         )
///         .with_file_output(
///             Settings::builder()
///                 .with_environment_variable("TEST_FILE_LOG")
///                 .with_default_level(LevelFilter::INFO)
///                 .file_log_settings_builder("/tmp/logs", "tracing-rs.log")
///                 .build()
///         )
///         .with_otlp_log_exporter(otlp_log_flag.then(|| {
///             Settings::builder()
///                 .with_environment_variable("TEST_OTLP_LOG")
///                 .with_default_level(LevelFilter::DEBUG)
///                 .build()
///         }))
///         .with_otlp_trace_exporter(
///             Settings::builder()
///                 .with_environment_variable("TEST_OTLP_TRACE")
///                 .with_default_level(LevelFilter::TRACE)
///                 .build()
///         )
///         .build()
///         .init()?;
///
///     tracing::info!("log a message");
///
///     Ok(())
/// }
/// ```
///
/// # Additional Configuration
///
/// You can configure the OTLP trace and log exports through the variables defined in the opentelemetry crates:
///
/// - `OTEL_EXPORTER_OTLP_COMPRESSION` (defaults to none, but can be set to `gzip`).
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` (defaults to `http://localhost:4317`, with the `grpc-tonic` feature (default)).
/// - `OTEL_EXPORTER_OTLP_TIMEOUT`
/// - `OTEL_EXPORTER_OTLP_HEADERS`
///
/// _See the defaults in the [opentelemetry-otlp][2] crate._
///
/// ## Tracing exporter overrides
///
/// OTLP Exporter settings:
///
/// - `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`
/// - `OTEL_EXPORTER_OTLP_TRACES_TIMEOUT`
/// - `OTEL_EXPORTER_OTLP_TRACES_COMPRESSION`
/// - `OTEL_EXPORTER_OTLP_TRACES_HEADERS`
///
/// General Span and Trace settings:
///
/// - `OTEL_SPAN_ATTRIBUTE_COUNT_LIMIT`
/// - `OTEL_SPAN_EVENT_COUNT_LIMIT`
/// - `OTEL_SPAN_LINK_COUNT_LIMIT`
/// - `OTEL_TRACES_SAMPLER` (Defaults to `parentbased_always_on`. If "traceidratio" or "parentbased_traceidratio", then `OTEL_TRACES_SAMPLER_ARG`)
///
/// Batch Span Processor settings:
///
/// - `OTEL_BSP_MAX_QUEUE_SIZE`
/// - `OTEL_BSP_SCHEDULE_DELAY`
/// - `OTEL_BSP_MAX_EXPORT_BATCH_SIZE`
/// - `OTEL_BSP_EXPORT_TIMEOUT`
/// - `OTEL_BSP_MAX_CONCURRENT_EXPORTS`
///
/// _See defaults in the opentelemetry_sdk crate under [trace::config][3] and [trace::span_processor][4]._
///
/// ## Log exporter overrides
///
/// OTLP exporter settings:
///
/// - `OTEL_EXPORTER_OTLP_LOGS_COMPRESSION`
/// - `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`
/// - `OTEL_EXPORTER_OTLP_LOGS_TIMEOUT`
/// - `OTEL_EXPORTER_OTLP_LOGS_HEADERS`
///
/// Batch Log Record Processor settings:
///
/// - `OTEL_BLRP_MAX_QUEUE_SIZE`
/// - `OTEL_BLRP_SCHEDULE_DELAY`
/// - `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE`
/// - `OTEL_BLRP_EXPORT_TIMEOUT`
///
/// _See defaults in the opentelemetry_sdk crate under [log::log_processor][5]._
///
/// [1]: tracing::Subscriber
/// [2]: https://docs.rs/opentelemetry-otlp/latest/src/opentelemetry_otlp/exporter/mod.rs.html
/// [3]: https://docs.rs/opentelemetry_sdk/latest/src/opentelemetry_sdk/trace/config.rs.html
/// [4]: https://docs.rs/opentelemetry_sdk/latest/src/opentelemetry_sdk/trace/span_processor.rs.html
/// [5]: https://docs.rs/opentelemetry_sdk/latest/src/opentelemetry_sdk/logs/log_processor.rs.html
pub struct Tracing {
    service_name: &'static str,
    console_log_settings: ConsoleLogSettings,
    file_log_settings: FileLogSettings,
    otlp_log_settings: OtlpLogSettings,
    otlp_trace_settings: OtlpTraceSettings,

    logger_provider: Option<SdkLoggerProvider>,
    tracer_provider: Option<SdkTracerProvider>,
}

impl Tracing {
    /// The environment variable used to set the console log level filter.
    pub const CONSOLE_LOG_ENV_VAR: &str = "CONSOLE_LOG";
    /// The environment variable used to set the rolling file log level filter.
    pub const FILE_LOG_ENV_VAR: &str = "FILE_LOG";
    /// The filename used for the rolling file logs.
    pub const FILE_LOG_NAME: &str = "operator.log";
    /// The environment variable used to set the OTLP log level filter.
    pub const OTLP_LOG_ENV_VAR: &str = "OTLP_LOG";
    /// The environment variable used to set the OTLP trace level filter.
    pub const OTLP_TRACE_ENV_VAR: &str = "OTLP_TRACE";

    /// Creates and returns a [`TracingBuilder`].
    pub fn builder() -> TracingBuilder<builder_state::PreServiceName> {
        TracingBuilder::default()
    }

    /// Creates an returns a pre-configured [`Tracing`] instance which can be initialized by
    /// calling [`Tracing::init()`].
    ///
    /// If `rolling_logs_period` is [`None`], this function will use a default value of
    /// [`RollingPeriod::Never`].
    pub fn pre_configured(service_name: &'static str, options: TelemetryOptions) -> Self {
        let TelemetryOptions {
            no_console_output,
            rolling_logs,
            rolling_logs_period,
            otlp_traces,
            otlp_logs,
        } = options;

        let rolling_logs_period = rolling_logs_period.unwrap_or_default();

        Self::builder()
            .service_name(service_name)
            .with_console_output((
                Self::CONSOLE_LOG_ENV_VAR,
                LevelFilter::INFO,
                !no_console_output,
            ))
            .with_file_output(rolling_logs.map(|log_directory| {
                Settings::builder()
                    .with_environment_variable(Self::FILE_LOG_ENV_VAR)
                    .with_default_level(LevelFilter::INFO)
                    .file_log_settings_builder(log_directory, Self::FILE_LOG_NAME)
                    .with_rotation_period(rolling_logs_period)
                    .build()
            }))
            .with_otlp_log_exporter((Self::OTLP_LOG_ENV_VAR, LevelFilter::DEBUG, otlp_logs))
            .with_otlp_trace_exporter((Self::OTLP_TRACE_ENV_VAR, LevelFilter::DEBUG, otlp_traces))
            .build()
    }

    /// Initialise the configured tracing subscribers, returning a guard that
    /// will shutdown the subscribers when dropped.
    ///
    /// <div class="warning">
    /// Name the guard variable appropriately, do not just use <code>let _ =</code>, as that will drop
    /// immediately.
    /// </div>
    pub fn init(mut self) -> Result<Tracing> {
        let mut layers: Vec<Box<dyn Layer<Registry> + Sync + Send>> = Vec::new();

        if let ConsoleLogSettings::Enabled {
            common_settings,
            log_format: _,
        } = &self.console_log_settings
        {
            let env_filter_layer = env_filter_builder(
                common_settings.environment_variable,
                common_settings.default_level,
            );
            let console_output_layer =
                tracing_subscriber::fmt::layer().with_filter(env_filter_layer);
            layers.push(console_output_layer.boxed());
        }

        if let FileLogSettings::Enabled {
            common_settings,
            file_log_dir,
            rotation_period,
            filename_suffix,
            max_log_files,
        } = &self.file_log_settings
        {
            let env_filter_layer = env_filter_builder(
                common_settings.environment_variable,
                common_settings.default_level,
            );

            let file_appender = RollingFileAppender::builder()
                .rotation(rotation_period.clone())
                .filename_prefix(self.service_name.to_string())
                .filename_suffix(filename_suffix);

            let file_appender = if let Some(max_log_files) = max_log_files {
                file_appender.max_log_files(*max_log_files)
            } else {
                file_appender
            };

            let file_appender = file_appender
                .build(file_log_dir)
                .context(InitRollingFileAppenderSnafu)?;

            layers.push(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_appender)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
        }

        if let OtlpLogSettings::Enabled { common_settings } = &self.otlp_log_settings {
            let env_filter_layer = env_filter_builder(
                common_settings.environment_variable,
                common_settings.default_level,
            )
            // TODO (@NickLarsenNZ): Remove this directive once https://github.com/open-telemetry/opentelemetry-rust/issues/761 is resolved
            .add_directive("h2=off".parse().expect("invalid directive"));

            let log_exporter = LogExporter::builder()
                .with_tonic()
                .build()
                .context(InstallOtelLogExporterSnafu)?;

            let logger_provider = SdkLoggerProvider::builder()
                .with_batch_exporter(log_exporter)
                .with_resource(
                    Resource::builder()
                        .with_service_name(self.service_name)
                        .build(),
                )
                .build();

            // Convert `tracing::Event` to OpenTelemetry logs
            layers.push(
                OpenTelemetryTracingBridge::new(&logger_provider)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
            self.logger_provider = Some(logger_provider);
        }

        if let OtlpTraceSettings::Enabled { common_settings } = &self.otlp_trace_settings {
            let env_filter_layer = env_filter_builder(
                // todo, deref?
                common_settings.environment_variable,
                common_settings.default_level,
            )
            // TODO (@NickLarsenNZ): Remove this directive once https://github.com/open-telemetry/opentelemetry-rust/issues/761 is resolved
            .add_directive("h2=off".parse().expect("invalid directive"));

            let trace_exporter = SpanExporter::builder()
                .with_tonic()
                .build()
                .context(InstallOtelTraceExporterSnafu)?;

            let tracer_provider = SdkTracerProvider::builder()
                .with_batch_exporter(trace_exporter)
                .with_resource(
                    Resource::builder()
                        .with_service_name(self.service_name)
                        .build(),
                )
                .build();

            let tracer = tracer_provider.tracer(self.service_name);

            layers.push(
                tracing_opentelemetry::layer()
                    .with_tracer(tracer)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
            self.tracer_provider = Some(tracer_provider);

            opentelemetry::global::set_text_map_propagator(
                // NOTE (@NickLarsenNZ): There are various propagators. Eg: TraceContextPropagator
                // standardises HTTP headers to propagate trace-id, parent-id, etc... while the
                // BaggagePropagator sets a "baggage" header with the value being key=value pairs. There
                // are other kinds too. There is also B3 and Jaeger, and some legacy stuff like OT Trace
                // and OpenCensus.
                // See: https://opentelemetry.io/docs/specs/otel/context/api-propagators/
                TraceContextPropagator::new(),
            );
        }

        if !layers.is_empty() {
            // Add the layers to the tracing_subscriber Registry (console,
            // tracing (OTLP), logging (OTLP))
            tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layers))
                .context(SetGlobalDefaultSubscriberSnafu)?;
        }

        // IMPORTANT: we must return self, otherwise Drop will be called and uninitialise tracing
        Ok(self)
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        tracing::debug!(
            opentelemetry.tracing.enabled = self.otlp_trace_settings.is_enabled(),
            opentelemetry.logger.enabled = self.otlp_log_settings.is_enabled(),
            "shutting down opentelemetry OTLP providers"
        );

        if let Some(tracer_provider) = &self.tracer_provider {
            if let Err(error) = tracer_provider.shutdown() {
                tracing::error!(%error, "unable to shutdown TracerProvider")
            }
        }

        if let Some(logger_provider) = &self.logger_provider {
            if let Err(error) = logger_provider.shutdown() {
                tracing::error!(%error, "unable to shutdown LoggerProvider");
            }
        }
    }
}

/// This trait is only used for the typestate builder and cannot be implemented
/// outside of this crate.
///
/// The only reason it has pub visibility is because it needs to be at least as
/// visible as the types that use it.
#[doc(hidden)]
pub trait BuilderState: private::Sealed {}

/// This private module holds the [`Sealed`][1] trait that is used by the
/// [`BuilderState`], so that it cannot be implemented outside of this crate.
///
/// We impl Sealed for any types that will use the trait that we want to
/// restrict impls on. In this case, the [`BuilderState`] trait.
///
/// [1]: private::Sealed
#[doc(hidden)]
mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for builder_state::PreServiceName {}
    impl Sealed for builder_state::Config {}
}

/// This module holds the possible states that the builder is in.
///
/// Each state will implement [`BuilderState`] (with no methods), and the
/// Builder struct ([`TracingBuilder`]) itself will be implemented with
/// each state as a generic parameter.
/// This allows only the methods to be called when the builder is in the
/// applicable state.
#[doc(hidden)]
mod builder_state {
    /// The initial state, before the service name is set.
    #[derive(Default)]
    pub struct PreServiceName;

    /// The state that allows you to configure the supported [`Subscriber`][1]
    /// [`Layer`][2].
    ///
    /// [1]: tracing::Subscriber
    /// [2]: tracing_subscriber::layer::Layer
    #[derive(Default)]
    pub struct Config;
}

// Make the states usable
#[doc(hidden)]
impl BuilderState for builder_state::PreServiceName {}

#[doc(hidden)]
impl BuilderState for builder_state::Config {}

/// Makes it easy to build a valid [`Tracing`] instance.
#[derive(Default)]
pub struct TracingBuilder<S: BuilderState> {
    service_name: Option<&'static str>,
    console_log_settings: ConsoleLogSettings,
    file_log_settings: FileLogSettings,
    otlp_log_settings: OtlpLogSettings,
    otlp_trace_settings: OtlpTraceSettings,

    /// Allow the generic to be used (needed for impls).
    _marker: std::marker::PhantomData<S>,
}

impl TracingBuilder<builder_state::PreServiceName> {
    /// Set the service name used in OTLP exports, and console output.
    ///
    /// A service name is required for valid OTLP telemetry.
    pub fn service_name(self, service_name: &'static str) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: Some(service_name),
            ..Default::default()
        }
    }
}

impl TracingBuilder<builder_state::Config> {
    /// Enable the console output tracing subscriber and set the default
    /// [`LevelFilter`][1] which is overridable through the given environment
    /// variable.
    ///
    /// [1]: tracing_subscriber::filter::LevelFilter
    pub fn with_console_output(
        self,
        console_log_settings: impl Into<ConsoleLogSettings>,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: console_log_settings.into(),
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings: self.otlp_trace_settings,
            file_log_settings: self.file_log_settings,
            _marker: self._marker,
        }
    }

    /// Enable the file output tracing subscriber and set the default
    /// [`LevelFilter`][1] which is overridable through the given environment
    /// variable.
    ///
    /// [1]: tracing_subscriber::filter::LevelFilter
    pub fn with_file_output(
        self,
        file_log_settings: impl Into<FileLogSettings>,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            file_log_settings: file_log_settings.into(),
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings: self.otlp_trace_settings,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP logging subscriber and set the default [`LevelFilter`][1]
    /// which is overridable through the given environment variable.
    ///
    /// You can configure the OTLP log exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    ///
    /// [1]: tracing_subscriber::filter::LevelFilter
    pub fn with_otlp_log_exporter(
        self,
        otlp_log_settings: impl Into<OtlpLogSettings>,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            otlp_log_settings: otlp_log_settings.into(),
            otlp_trace_settings: self.otlp_trace_settings,
            file_log_settings: self.file_log_settings,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP tracing subscriber and set the default [`LevelFilter`][1]
    /// which is overridable through the given environment variable.
    ///
    /// You can configure the OTLP trace exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    ///
    /// [1]: tracing_subscriber::filter::LevelFilter
    pub fn with_otlp_trace_exporter(
        self,
        otlp_trace_settings: impl Into<OtlpTraceSettings>,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings: otlp_trace_settings.into(),
            file_log_settings: self.file_log_settings,
            _marker: self._marker,
        }
    }

    /// Consumes self and returns a valid [`Tracing`] instance.
    ///
    /// Once built, you can call [`Tracing::init`] to enable the configured
    /// tracing subscribers.
    pub fn build(self) -> Tracing {
        Tracing {
            service_name: self
                .service_name
                .expect("service_name must be configured at this point"),
            console_log_settings: self.console_log_settings,
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings: self.otlp_trace_settings,
            file_log_settings: self.file_log_settings,
            logger_provider: None,
            tracer_provider: None,
        }
    }
}

/// Create an [`EnvFilter`] configured with the given environment variable and default [`Directive`].
fn env_filter_builder(env_var: &str, default_directive: impl Into<Directive>) -> EnvFilter {
    EnvFilter::builder()
        .with_env_var(env_var)
        .with_default_directive(default_directive.into())
        .from_env_lossy()
}

/// Contains options which can be passed to [`Tracing::pre_configured()`].
///
/// Additionally, this struct can be used as operator CLI arguments. This functionality is only
/// available if the feature `clap` is enabled.
#[cfg_attr(feature = "clap", derive(clap::Args, PartialEq, Eq))]
#[derive(Debug, Default)]
pub struct TelemetryOptions {
    /// Disable console output.
    #[cfg_attr(feature = "clap", arg(long, env))]
    pub no_console_output: bool,

    /// Enable logging to rolling files located in the specified DIRECTORY.
    #[cfg_attr(
        feature = "clap",
        arg(
            long,
            env = "ROLLING_LOGS_DIR",
            value_name = "DIRECTORY",
            group = "rolling_logs_group"
        )
    )]
    pub rolling_logs: Option<PathBuf>,

    /// Time PERIOD after which log files are rolled over.
    #[cfg_attr(
        feature = "clap",
        arg(long, env, value_name = "PERIOD", requires = "rolling_logs_group")
    )]
    pub rolling_logs_period: Option<RollingPeriod>,

    /// Enable exporting traces via OTLP.
    #[cfg_attr(feature = "clap", arg(long, env))]
    pub otlp_traces: bool,

    /// Enable exporting logs via OTLP.
    #[cfg_attr(feature = "clap", arg(long, env))]
    pub otlp_logs: bool,
}

/// Supported periods when the log file is rolled over.
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[derive(Clone, Debug, Default, PartialEq, Eq, strum::Display, strum::EnumString)]
#[allow(missing_docs)]
pub enum RollingPeriod {
    Minutely,
    Hourly,
    Daily,

    #[default]
    Never,
}

impl From<RollingPeriod> for Rotation {
    fn from(value: RollingPeriod) -> Self {
        match value {
            RollingPeriod::Minutely => Self::MINUTELY,
            RollingPeriod::Hourly => Self::HOURLY,
            RollingPeriod::Daily => Self::DAILY,
            RollingPeriod::Never => Self::NEVER,
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use rstest::rstest;
    use settings::Settings;
    use tracing::level_filters::LevelFilter;
    use tracing_appender::rolling::Rotation;

    use super::*;

    #[test]
    fn builder_basic_construction() {
        let trace_guard = Tracing::builder().service_name("test").build();

        assert_eq!(trace_guard.service_name, "test");
    }

    #[test]
    fn builder_with_console_output() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(
                Settings::builder()
                    .with_environment_variable("ABC_A")
                    .with_default_level(LevelFilter::TRACE)
                    .build(),
            )
            .with_console_output(
                Settings::builder()
                    .with_environment_variable("ABC_B")
                    .with_default_level(LevelFilter::DEBUG)
                    .build(),
            )
            .build();

        assert_eq!(
            trace_guard.console_log_settings,
            ConsoleLogSettings::Enabled {
                common_settings: Settings {
                    environment_variable: "ABC_B",
                    default_level: LevelFilter::DEBUG
                },
                log_format: Default::default()
            }
        );

        assert!(trace_guard.file_log_settings.is_disabled());
        assert!(trace_guard.otlp_log_settings.is_disabled());
        assert!(trace_guard.otlp_trace_settings.is_disabled());
    }

    #[test]
    fn builder_with_console_output_double() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(("ABC_A", LevelFilter::TRACE))
            .build();

        assert_eq!(
            trace_guard.console_log_settings,
            ConsoleLogSettings::Enabled {
                common_settings: Settings {
                    environment_variable: "ABC_A",
                    default_level: LevelFilter::TRACE,
                },
                log_format: Default::default()
            }
        )
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn builder_with_console_output_triple(#[case] enabled: bool) {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(("ABC_A", LevelFilter::TRACE, enabled))
            .build();

        let expected = match enabled {
            true => ConsoleLogSettings::Enabled {
                common_settings: Settings {
                    environment_variable: "ABC_A",
                    default_level: LevelFilter::TRACE,
                },
                log_format: Default::default(),
            },
            false => ConsoleLogSettings::Disabled,
        };

        assert_eq!(trace_guard.console_log_settings, expected)
    }

    #[test]
    fn builder_with_all() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(
                Settings::builder()
                    .with_environment_variable("ABC_CONSOLE")
                    .with_default_level(LevelFilter::INFO)
                    .build(),
            )
            .with_file_output(
                Settings::builder()
                    .with_environment_variable("ABC_FILE")
                    .with_default_level(LevelFilter::INFO)
                    .file_log_settings_builder(PathBuf::from("/abc_file_dir"), "tracing-rs.json")
                    .build(),
            )
            .with_otlp_log_exporter(
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_LOG")
                    .with_default_level(LevelFilter::DEBUG)
                    .build(),
            )
            .with_otlp_trace_exporter(
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_TRACE")
                    .with_default_level(LevelFilter::TRACE)
                    .build(),
            )
            .build();

        assert_eq!(
            trace_guard.console_log_settings,
            ConsoleLogSettings::Enabled {
                common_settings: Settings {
                    environment_variable: "ABC_CONSOLE",
                    default_level: LevelFilter::INFO
                },
                log_format: Default::default()
            }
        );
        assert_eq!(trace_guard.file_log_settings, FileLogSettings::Enabled {
            common_settings: Settings {
                environment_variable: "ABC_FILE",
                default_level: LevelFilter::INFO
            },
            file_log_dir: PathBuf::from("/abc_file_dir"),
            rotation_period: Rotation::NEVER,
            filename_suffix: "tracing-rs.json".to_owned(),
            max_log_files: None,
        });
        assert_eq!(trace_guard.otlp_log_settings, OtlpLogSettings::Enabled {
            common_settings: Settings {
                environment_variable: "ABC_OTLP_LOG",
                default_level: LevelFilter::DEBUG
            },
        });
        assert_eq!(
            trace_guard.otlp_trace_settings,
            OtlpTraceSettings::Enabled {
                common_settings: Settings {
                    environment_variable: "ABC_OTLP_TRACE",
                    default_level: LevelFilter::TRACE
                }
            }
        );
    }

    #[test]
    fn builder_with_options() {
        let enable_console_output = true;
        let enable_filelog_output = true;
        let enable_otlp_trace = true;
        let enable_otlp_log = false;

        let tracing_builder = Tracing::builder()
            .service_name("test")
            .with_console_output(enable_console_output.then(|| {
                Settings::builder()
                    .with_environment_variable("ABC_CONSOLE")
                    .build()
            }))
            .with_file_output(enable_filelog_output.then(|| {
                Settings::builder()
                    .with_environment_variable("ABC_FILELOG")
                    .file_log_settings_builder("/dev/null", "tracing-rs.json")
                    .build()
            }))
            .with_otlp_trace_exporter(enable_otlp_trace.then(|| {
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_TRACE")
                    .build()
            }))
            .with_otlp_log_exporter(enable_otlp_log.then(|| {
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_LOG")
                    .build()
            }))
            .build();


    #[test]
    fn pre_configured() {
        let tracing = Tracing::pre_configured("test", TelemetryOptions {
            no_console_output: false,
            rolling_logs: None,
            rolling_logs_period: None,
            otlp_traces: true,
            otlp_logs: false,
        });

        assert!(tracing.otlp_trace_settings.is_enabled());
    }
}
