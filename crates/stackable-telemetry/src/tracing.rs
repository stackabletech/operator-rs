//! This module contains functionality to initialise tracing Subscribers for
//! console output, and OpenTelemetry OTLP export for traces and logs.
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
use tracing::{level_filters::LevelFilter, subscriber::SetGlobalDefaultError};
use tracing_subscriber::{filter::Directive, layer::SubscriberExt, EnvFilter, Layer, Registry};

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
/// use stackable_telemetry::tracing::{Tracing, Error};
/// use tracing_subscriber::filter::LevelFilter;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     // IMPORTANT: Name the guard variable appropriately, do not just use
///     // `let _ =`, as that will drop immediately.
///     let _tracing_guard = Tracing::builder()
///         .service_name("test")
///         .with_console_output("TEST_CONSOLE", LevelFilter::INFO)
///         .with_otlp_log_exporter("TEST_OTLP_LOG", LevelFilter::DEBUG)
///         .with_otlp_trace_exporter("TEST_OTLP_TRACE", LevelFilter::TRACE)
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
    console_log_config: SubscriberConfig,
    otlp_log_config: SubscriberConfig,
    otlp_trace_config: SubscriberConfig,
    logger_provider: Option<LoggerProvider>,
}

impl Tracing {
    pub fn builder() -> TracingBuilder<builder_state::PreServiceName> {
        TracingBuilder::default()
    }

    /// Initialise the configured tracing subscribers, returning a guard that
    /// will shutdown the subscribers when dropped.
    pub fn init(mut self) -> Result<Tracing> {
        let mut layers: Vec<Box<dyn Layer<Registry> + Sync + Send>> = Vec::new();

        if self.console_log_config.enabled {
            let env_filter_layer = env_filter_builder(
                self.console_log_config.env_var,
                self.console_log_config.default_level_filter,
            );
            let console_output_layer =
                tracing_subscriber::fmt::layer().with_filter(env_filter_layer);
            layers.push(console_output_layer.boxed());
        }

        if self.otlp_log_config.enabled {
            let env_filter_layer = env_filter_builder(
                self.otlp_log_config.env_var,
                self.otlp_log_config.default_level_filter,
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

        if self.otlp_trace_config.enabled {
            let env_filter_layer = env_filter_builder(
                self.otlp_trace_config.env_var,
                self.otlp_trace_config.default_level_filter,
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
        // NOTE (@NickLarsenNZ): This might eventually be replaced with something like SdkMeterProvider::shutdown(&self)
        // as has been done with the LoggerProvider (further below)
        // see: https://github.com/open-telemetry/opentelemetry-rust/pull/1412/files#r1409608679
        opentelemetry::global::shutdown_tracer_provider();

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

    /// The state before the [`EnvFilter`][1] environment variable name is set.
    ///
    /// [1]: tracing_subscriber::filter::EnvFilter
    #[derive(Default)]
    pub struct PreEnvVar;

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
    console_log_config: SubscriberConfig,
    otlp_log_config: SubscriberConfig,
    otlp_trace_config: SubscriberConfig,

    /// Allow the generic to be used (needed for impls).
    _marker: std::marker::PhantomData<S>,
}

#[derive(Clone, Debug, PartialEq)]
struct SubscriberConfig {
    enabled: bool,
    env_var: &'static str,
    default_level_filter: LevelFilter,
}

impl Default for SubscriberConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            env_var: EnvFilter::DEFAULT_ENV,
            default_level_filter: LevelFilter::OFF,
        }
    }
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
    /// [`LevelFilter`] which is overridable through the given environment
    /// variable.
    pub fn with_console_output(
        self,
        env_var: &'static str,
        default_level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: SubscriberConfig {
                enabled: true,
                env_var,
                default_level_filter,
            },
            otlp_log_config: self.otlp_log_config,
            otlp_trace_config: self.otlp_trace_config,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP logging subscriber and set the default [`LevelFilter`]
    /// which is overridable through the given environment variable.
    ///
    /// You can configure the OTLP log exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    pub fn with_otlp_log_exporter(
        self,
        env_var: &'static str,
        default_level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: self.console_log_config,
            otlp_log_config: SubscriberConfig {
                enabled: true,
                env_var,
                default_level_filter,
            },
            otlp_trace_config: self.otlp_trace_config,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP tracing subscriber and set the default [`LevelFilter`]
    /// which is overridable through the given environment variable.
    ///
    /// You can configure the OTLP trace exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    pub fn with_otlp_trace_exporter(
        self,
        env_var: &'static str,
        default_level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: self.console_log_config,
            otlp_log_config: self.otlp_log_config,
            otlp_trace_config: SubscriberConfig {
                enabled: true,
                env_var,
                default_level_filter,
            },
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
            console_log_config: self.console_log_config,
            otlp_log_config: self.otlp_log_config,
            otlp_trace_config: self.otlp_trace_config,
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
            .with_console_output("ABC_A", LevelFilter::TRACE)
            .with_console_output("ABC_B", LevelFilter::DEBUG)
            .build();

        assert_eq!(
            trace_guard.console_log_config,
            SubscriberConfig {
                enabled: true,
                env_var: "ABC_B",
                default_level_filter: LevelFilter::DEBUG
            }
        );
        assert!(!trace_guard.otlp_log_config.enabled);
        assert!(!trace_guard.otlp_trace_config.enabled);
    }

    #[test]
    fn builder_with_all() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output("ABC_CONSOLE", LevelFilter::INFO)
            .with_otlp_log_exporter("ABC_OTLP_LOG", LevelFilter::DEBUG)
            .with_otlp_trace_exporter("ABC_OTLP_TRACE", LevelFilter::TRACE)
            .build();

        assert_eq!(
            trace_guard.console_log_config,
            SubscriberConfig {
                enabled: true,
                env_var: "ABC_CONSOLE",
                default_level_filter: LevelFilter::INFO
            }
        );
        assert_eq!(
            trace_guard.otlp_log_config,
            SubscriberConfig {
                enabled: true,
                env_var: "ABC_OTLP_LOG",
                default_level_filter: LevelFilter::DEBUG
            }
        );
        assert_eq!(
            trace_guard.otlp_trace_config,
            SubscriberConfig {
                enabled: true,
                env_var: "ABC_OTLP_TRACE",
                default_level_filter: LevelFilter::TRACE
            }
        );
    }
}
