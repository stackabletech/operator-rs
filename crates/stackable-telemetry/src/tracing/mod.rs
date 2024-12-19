//! This module contains functionality to initialise tracing Subscribers for
//! console output, file output and OpenTelemetry OTLP export for traces and logs.
//!
//! It is intended to be used by the Stackable Data Platform operators and
//! webhooks, but it should be generic enough to be used in any application.
//!
//! To get started, see [`Tracing`].

use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::{
    logs::{self, LoggerProvider},
    propagation::TraceContextPropagator,
    trace, Resource,
};
use opentelemetry_semantic_conventions::resource;
use snafu::{ResultExt as _, Snafu};
use tracing::subscriber::SetGlobalDefaultError;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{filter::Directive, layer::SubscriberExt, EnvFilter, Layer, Registry};

use settings::{ConsoleLogSettings, FileLogSettings, OtlpLogSettings, OtlpTraceSettings};

pub mod settings;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("unable to install opentelemetry trace exporter"))]
    InstallOtelTraceExporter {
        source: opentelemetry::trace::TraceError,
    },

    #[snafu(display("unable to install opentelemetry log exporter"))]
    InstallOtelLogExporter {
        source: opentelemetry::logs::LogError,
    },

    #[snafu(display("unable to set the global default subscriber"))]
    SetGlobalDefaultSubscriber { source: SetGlobalDefaultError },
}

/// Easily initialize a set of preconfigured [`Subscriber`][1] layers.
///
/// # Usage:
/// ```
/// use stackable_telemetry::tracing::{Tracing, Error, settings::{Build as _, Settings}};
/// use tracing_subscriber::filter::LevelFilter;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     // This can come from a Clap argument for example. The enabled builder
///     // function below allows enabling/disabling certain subscribers during
///     // runtime.
///     let otlp_log_flag = false;
///
///     // IMPORTANT: Name the guard variable appropriately, do not just use
///     // `let _ =`, as that will drop immediately.
///     let _tracing_guard = Tracing::builder()
///         .service_name("test")
///         .with_console_output(
///             Settings::builder()
///                 .with_environment_variable("TEST_CONSOLE")
///                 .with_default_level(LevelFilter::INFO)
///                 .enabled(true)
///                 .build()
///         )
///         .with_otlp_log_exporter(
///             Settings::builder()
///                 .with_environment_variable("TEST_OTLP_LOG")
///                 .with_default_level(LevelFilter::DEBUG)
///                 .enabled(otlp_log_flag)
///                 .build()
///         )
///         .with_otlp_trace_exporter(
///             Settings::builder()
///                 .with_environment_variable("TEST_OTLP_TRACE")
///                 .with_default_level(LevelFilter::TRACE)
///                 .enabled(true)
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
    logger_provider: Option<LoggerProvider>,
}

impl Tracing {
    pub fn builder() -> TracingBuilder<builder_state::PreServiceName> {
        TracingBuilder::default()
    }

    /// Initialise the configured tracing subscribers, returning a guard that
    /// will shutdown the subscribers when dropped.
    ///
    /// IMPORTANT: Name the guard variable appropriately, do not just use
    /// `let _ =`, as that will drop immediately.
    pub fn init(mut self) -> Result<Tracing> {
        let mut layers: Vec<Box<dyn Layer<Registry> + Sync + Send>> = Vec::new();

        if self.console_log_settings.enabled {
            let env_filter_layer = env_filter_builder(
                self.console_log_settings
                    .common_settings
                    .environment_variable,
                self.console_log_settings.default_level,
            );
            let console_output_layer =
                tracing_subscriber::fmt::layer().with_filter(env_filter_layer);
            layers.push(console_output_layer.boxed());
        }

        if self.file_log_settings.enabled {
            let env_filter_layer = env_filter_builder(
                self.file_log_settings.common_settings.environment_variable,
                self.file_log_settings.default_level,
            );

            let file_appender = RollingFileAppender::builder()
                .rotation(Rotation::HOURLY)
                .filename_prefix(self.service_name.to_string())
                .filename_suffix("tracing-rs.json")
                .max_log_files(6)
                .build(&self.file_log_settings.file_log_dir)
                .expect("failed to initialize rolling file appender");

            layers.push(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_appender)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
        }

        if self.otlp_log_settings.enabled {
            let env_filter_layer = env_filter_builder(
                self.otlp_log_settings.environment_variable,
                self.otlp_log_settings.default_level,
            )
            // TODO (@NickLarsenNZ): Remove this directive once https://github.com/open-telemetry/opentelemetry-rust/issues/761 is resolved
            .add_directive("h2=off".parse().expect("invalid directive"));

            let log_exporter = opentelemetry_otlp::new_exporter().tonic();
            let otel_log =
                opentelemetry_otlp::new_pipeline()
                    .logging()
                    .with_exporter(log_exporter)
                    .with_log_config(logs::config().with_resource(Resource::new(vec![
                        KeyValue::new(resource::SERVICE_NAME, self.service_name),
                    ])))
                    .install_batch(opentelemetry_sdk::runtime::Tokio)
                    .context(InstallOtelLogExporterSnafu)?;

            // Convert `tracing::Event` to OpenTelemetry logs
            layers.push(
                OpenTelemetryTracingBridge::new(&otel_log)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
            self.logger_provider = Some(otel_log);
        }

        if self.otlp_trace_settings.enabled {
            let env_filter_layer = env_filter_builder(
                self.otlp_trace_settings
                    .common_settings
                    .environment_variable,
                self.otlp_trace_settings.default_level,
            )
            // TODO (@NickLarsenNZ): Remove this directive once https://github.com/open-telemetry/opentelemetry-rust/issues/761 is resolved
            .add_directive("h2=off".parse().expect("invalid directive"));

            let trace_exporter = opentelemetry_otlp::new_exporter().tonic();
            let otel_tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(trace_exporter)
                .with_trace_config(trace::config().with_resource(Resource::new(vec![
                    KeyValue::new(resource::SERVICE_NAME, self.service_name),
                ])))
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .context(InstallOtelTraceExporterSnafu)?;

            layers.push(
                tracing_opentelemetry::layer()
                    .with_tracer(otel_tracer)
                    .with_filter(env_filter_layer)
                    .boxed(),
            );

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
            opentelemetry.tracing.enabled = self.otlp_trace_settings.enabled,
            opentelemetry.logger.enabled = self.otlp_log_settings.enabled,
            "shutting down opentelemetry OTLP providers"
        );

        if self.otlp_trace_settings.enabled {
            // NOTE (@NickLarsenNZ): This might eventually be replaced with something like SdkMeterProvider::shutdown(&self)
            // as has been done with the LoggerProvider (further below)
            // see: https://github.com/open-telemetry/opentelemetry-rust/pull/1412/files#r1409608679
            opentelemetry::global::shutdown_tracer_provider();
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
    otlp_log_settings: OtlpLogSettings,
    otlp_trace_settings: OtlpTraceSettings,
    file_log_settings: FileLogSettings,

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
        console_log_settings: ConsoleLogSettings,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings,
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
        file_log_settings: FileLogSettings,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            file_log_settings,
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings: self.otlp_trace_settings,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP logging subscriber and set the default [`LevelFilter`]
    /// which is overridable through the given environment variable.
    ///
    /// You can configure the OTLP log exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    ///
    /// [1]: tracing_subscriber::filter::LevelFilter
    pub fn with_otlp_log_exporter(
        self,
        otlp_log_settings: OtlpLogSettings,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            otlp_log_settings,
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
        otlp_trace_settings: OtlpTraceSettings,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_settings: self.console_log_settings,
            otlp_log_settings: self.otlp_log_settings,
            otlp_trace_settings,
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

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use settings::{Build as _, Settings};
    use tracing::level_filters::LevelFilter;

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
                    .enabled(true)
                    .build(),
            )
            .with_console_output(
                Settings::builder()
                    .with_environment_variable("ABC_B")
                    .with_default_level(LevelFilter::DEBUG)
                    .enabled(true)
                    .build(),
            )
            .build();

        assert_eq!(
            trace_guard.console_log_settings,
            ConsoleLogSettings {
                common_settings: Settings {
                    enabled: true,
                    environment_variable: "ABC_B",
                    default_level: LevelFilter::DEBUG
                },
                log_format: Default::default()
            }
        );
        assert!(!trace_guard.otlp_log_settings.enabled);
        assert!(!trace_guard.otlp_trace_settings.enabled);
    }

    #[test]
    fn builder_with_all() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(
                Settings::builder()
                    .with_environment_variable("ABC_CONSOLE")
                    .with_default_level(LevelFilter::INFO)
                    .enabled(true)
                    .build(),
            )
            .with_file_output(
                Settings::builder()
                    .with_environment_variable("ABC_FILE")
                    .with_default_level(LevelFilter::INFO)
                    .enabled(true)
                    .file_log_settings_builder()
                    .with_file_log_dir(String::from("/abc_file_dir"))
                    .build(),
            )
            .with_otlp_log_exporter(
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_LOG")
                    .with_default_level(LevelFilter::DEBUG)
                    .enabled(true)
                    .build(),
            )
            .with_otlp_trace_exporter(
                Settings::builder()
                    .with_environment_variable("ABC_OTLP_TRACE")
                    .with_default_level(LevelFilter::TRACE)
                    .enabled(true)
                    .build(),
            )
            .build();

        assert_eq!(
            trace_guard.console_log_settings,
            ConsoleLogSettings {
                common_settings: Settings {
                    enabled: true,
                    environment_variable: "ABC_CONSOLE",
                    default_level: LevelFilter::INFO
                },
                log_format: Default::default()
            }
        );
        assert_eq!(
            trace_guard.file_log_settings,
            FileLogSettings {
                common_settings: Settings {
                    enabled: true,
                    environment_variable: "ABC_FILE",
                    default_level: LevelFilter::INFO
                },
                file_log_dir: PathBuf::from("/abc_file_dir")
            }
        );
        assert_eq!(
            trace_guard.otlp_log_settings,
            OtlpLogSettings {
                common_settings: Settings {
                    enabled: true,
                    environment_variable: "ABC_OTLP_LOG",
                    default_level: LevelFilter::DEBUG
                },
            }
        );
        assert_eq!(
            trace_guard.otlp_trace_settings,
            OtlpTraceSettings {
                common_settings: Settings {
                    enabled: true,
                    environment_variable: "ABC_OTLP_TRACE",
                    default_level: LevelFilter::TRACE
                }
            }
        );
    }
}
