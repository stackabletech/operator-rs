use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::{
    logs,
    propagation::TraceContextPropagator,
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions::resource;
use snafu::{ResultExt as _, Snafu};
use tracing::{level_filters::LevelFilter, subscriber::SetGlobalDefaultError};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer, Registry};

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

    #[snafu(display(
        "unable to set the Log implementation that would convert log::Record as trace::Event"
    ))]
    InitLogTracer {
        source: tracing_log::log::SetLoggerError,
    },
}

/// Usage:
/// ```
/// use stackable_telemetry::tracing::{Tracing, Error};
/// use tracing_subscriber::filter::LevelFilter;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let _tracing_guard = Tracing::builder()
///         .service_name("test")
///         .with_console_output(LevelFilter::TRACE)
///         .with_otlp_log_exporter(LevelFilter::DEBUG)
///         .with_otlp_trace_exporter(LevelFilter::INFO)
///         .build()
///         .init()?;
///
///     tracing::info!("log a message");
///
///     Ok(())
/// }
/// ```
///
/// You can configure the OTLP trace exports through the variables defined in the opentelemetry crates:
/// - opentelemetry-otlp: `OTEL_EXPORTER_OTLP_COMPRESSION`
/// - opentelemetry-otlp: `OTEL_EXPORTER_OTLP_ENDPOINT` (defaults to: `http://localhost:4317 `)
/// - opentelemetry-otlp: `OTEL_EXPORTER_OTLP_TIMEOUT`
/// - opentelemetry-otlp: `OTEL_EXPORTER_OTLP_HEADERS`
/// - opentelemetry-sdk: `OTEL_SPAN_ATTRIBUTE_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_SPAN_EVENT_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_SPAN_LINK_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_TRACES_SAMPLER` (if "traceidratio" or "parentbased_traceidratio", then `OTEL_TRACES_SAMPLER_ARG`)
/// - opentelemetry-sdk: `OTEL_BSP_MAX_QUEUE_SIZE`
/// - opentelemetry-sdk: `OTEL_BSP_SCHEDULE_DELAY`
/// - opentelemetry-sdk: `OTEL_BSP_MAX_EXPORT_BATCH_SIZE`
/// - opentelemetry-sdk: `OTEL_BSP_EXPORT_TIMEOUT`
/// - opentelemetry-sdk: `OTEL_BSP_MAX_CONCURRENT_EXPORTS`
/// - opentelemetry-sdk: `OTEL_BLRP_MAX_QUEUE_SIZE`
/// - opentelemetry-sdk: `OTEL_BLRP_SCHEDULE_DELAY`
/// - opentelemetry-sdk: `OTEL_BLRP_MAX_EXPORT_BATCH_SIZE`
/// - opentelemetry-sdk: `OTEL_BLRP_EXPORT_TIMEOUT`
///
/// todo: add exporter specific vars here
pub struct Tracing {
    service_name: &'static str,
    console_log_config: SubscriberConfig,
    otlp_log_config: SubscriberConfig,
    otlp_trace_config: SubscriberConfig,
}

impl Tracing {
    pub fn builder() -> TracingBuilder<builder_state::PreServiceName> {
        TracingBuilder::default()
    }

    // HDFS_LOG_LEVEL=trace,h2=warn

    /// Initialise the configured tracing subscribers, returning a guard that
    /// will shutdown the subscribers when dropped.
    pub fn init(self) -> Result<Tracing> {
        let mut layers: Vec<Box<dyn Layer<Registry> + Sync + Send>> = Vec::new();

        if self.console_log_config.enabled {
            let env_filter_layer = EnvFilter::builder()
                .with_default_directive(self.console_log_config.level_filter.into()) // TODO (@NickLarsenNZ): support Directives
                .from_env_lossy();
            let console_output_layer =
                tracing_subscriber::fmt::layer().with_filter(env_filter_layer);
            layers.push(console_output_layer.boxed());
        }

        if self.otlp_log_config.enabled {
            tracing_log::LogTracer::init().context(InitLogTracerSnafu)?;

            let env_filter_layer = EnvFilter::builder()
                .with_default_directive(self.otlp_log_config.level_filter.into()) // TODO (@NickLarsenNZ): support Directives
                .from_env_lossy();

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

            // Covert `tracing::Event` to OpenTelemetry logs. `log::Record`s
            // will already be converted to `tracing::Event` by the `tacing-log`
            // crate with the `log-tracer` feature.
            layers.push(
                OpenTelemetryTracingBridge::new(otel_log.provider())
                    .with_filter(env_filter_layer)
                    .boxed(),
            );
        }

        if self.otlp_trace_config.enabled {
            let env_filter_layer = EnvFilter::builder()
                .with_default_directive(self.otlp_trace_config.level_filter.into()) // TODO (@NickLarsenNZ): support Directives
                .from_env_lossy();
            // .add_directive("hyper=info".parse().expect("invalid directive"))
            // .add_directive("tonic=warn".parse().expect("invalid directive"))
            // .add_directive("tokio_util=warn".parse().expect("invalid directive"))
            // .add_directive("hyper=info".parse().expect("invalid directive"))
            // .add_directive("h2=info".parse().expect("invalid directive"))
            // .add_directive("tower=info".parse().expect("invalid directive"));

            let trace_exporter = opentelemetry_otlp::new_exporter().tonic();
            let otel_tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(trace_exporter)
                .with_trace_config(
                    trace::config()
                        .with_sampler(Sampler::AlwaysOn) // TODO (@NickLarsenNZ): Make this configurable. See also Sampler::ParentBased
                        .with_id_generator(RandomIdGenerator::default()) // TODO (@NickLarsenNZ): Is there a more appropriate ID generator?
                        .with_resource(Resource::new(vec![KeyValue::new(
                            resource::SERVICE_NAME,
                            self.service_name,
                        )])),
                )
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
            // tracing (OLTP), logging (OLTP))
            tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layers))
                .context(SetGlobalDefaultSubscriberSnafu)?;
        }

        Ok(self)
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        // NOTE (@NickLarsenNZ): These might eventually be replaced with something like SdkMeterProvider::shutdown(&self)
        // see: https://github.com/open-telemetry/opentelemetry-rust/pull/1412/files#r1409608679
        opentelemetry::global::shutdown_tracer_provider();
        opentelemetry::global::shutdown_logger_provider();
    }
}

/// This trait is only used for the typestate builder and cannot be implemented
/// outside of this crate.
///
/// The only reason it has pub visibility is because it needs to be at least as
/// visible as the types that use it.
#[doc(hidden)]
pub trait BuilderState: private::Sealed {}

/// This private module holds the [`Sealed`] trait that is used by the
/// [`BuilderState`], so that is cannot be implemented outside of this crate.
///
/// We impl Sealed for any types that will use the trait that we want to
/// restrict impls on. In this case, the [`BuilderState`] trait.
#[doc(hidden)]
mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for builder_state::PreServiceName {}
    impl Sealed for builder_state::Config {}
}

/// This module holds the possible states that the builder is in.
///
/// Each state will implement [`super::BuilderState`] (with no methods), and the
/// Builder struct ([`super::TracingBuilder`]) itself will be implemented with
/// each state as a generic parameter.
/// This allows only the methods to be called when the builder is in the the
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

// Make the state valid
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
    level_filter: LevelFilter,
}

impl Default for SubscriberConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            level_filter: LevelFilter::OFF,
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
    /// Enable the console output tracing subscriber, and filter the log level.
    pub fn with_console_output(
        self,
        level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: SubscriberConfig {
                enabled: true,
                level_filter,
            },
            otlp_log_config: self.otlp_log_config,
            otlp_trace_config: self.otlp_trace_config,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP logging subscriber, and filter the log level.
    ///
    /// You can configure the OTLP log exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    pub fn with_otlp_log_exporter(
        self,
        level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: self.console_log_config,
            otlp_log_config: SubscriberConfig {
                enabled: true,
                level_filter,
            },
            otlp_trace_config: self.otlp_trace_config,
            _marker: self._marker,
        }
    }

    /// Enable the OTLP tracing subscriber, and filter the log level.
    ///
    /// You can configure the OTLP trace exports through the variables defined
    /// in the opentelemetry crates. See [`Tracing`].
    pub fn with_otlp_trace_exporter(
        self,
        level_filter: LevelFilter,
    ) -> TracingBuilder<builder_state::Config> {
        TracingBuilder {
            service_name: self.service_name,
            console_log_config: self.console_log_config,
            otlp_log_config: self.otlp_log_config,
            otlp_trace_config: SubscriberConfig {
                enabled: true,
                level_filter,
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
        }
    }
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
            .with_console_output(LevelFilter::TRACE)
            .with_console_output(LevelFilter::DEBUG)
            .build();

        assert_eq!(
            trace_guard.console_log_config,
            SubscriberConfig {
                enabled: true,
                level_filter: LevelFilter::DEBUG
            }
        );
        assert!(!trace_guard.otlp_log_config.enabled);
        assert!(!trace_guard.otlp_trace_config.enabled);
    }

    #[test]
    fn builder_with_all() {
        let trace_guard = Tracing::builder()
            .service_name("test")
            .with_console_output(LevelFilter::TRACE)
            .with_otlp_log_exporter(LevelFilter::DEBUG)
            .with_otlp_trace_exporter(LevelFilter::INFO)
            .build();

        assert_eq!(
            trace_guard.console_log_config,
            SubscriberConfig {
                enabled: true,
                level_filter: LevelFilter::TRACE
            }
        );
        assert_eq!(
            trace_guard.otlp_log_config,
            SubscriberConfig {
                enabled: true,
                level_filter: LevelFilter::DEBUG
            }
        );
        assert_eq!(
            trace_guard.otlp_trace_config,
            SubscriberConfig {
                enabled: true,
                level_filter: LevelFilter::INFO
            }
        );
    }
}
