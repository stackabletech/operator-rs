use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{self, RandomIdGenerator},
    Resource,
};
use opentelemetry_semantic_conventions::resource;
use snafu::{ResultExt as _, Snafu};
use tracing::{level_filters::LevelFilter, subscriber::SetGlobalDefaultError};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer as _};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("unable to install opentelemetry span exporter"))]
    InstallOtelExporter {
        source: opentelemetry::trace::TraceError,
    },

    #[snafu(display("unable to set the global default subscriber"))]
    SetGlobalDefaultSubscriber { source: SetGlobalDefaultError },
}

/// Usage:
/// ```
/// use stackable_telemetry::tracing::{Tracing, Error};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let _trace_guard = Tracing::init("my-service")?;
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
/// - opentelemetry-sdk: `OTEL_SERVICE_NAME` (we will make this hard coded in the operator)
/// - opentelemetry-sdk: `OTEL_SPAN_ATTRIBUTE_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_SPAN_EVENT_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_SPAN_LINK_COUNT_LIMIT`
/// - opentelemetry-sdk: `OTEL_TRACES_SAMPLER` (if "traceidratio" or "parentbased_traceidratio", then `OTEL_TRACES_SAMPLER_ARG`)
// TODO (@NickLarsenNZ): Link to the defaults in the opentelemetry-rust crates
pub struct Tracing;

impl Tracing {
    /// The TracerProvider will shutdown when the result is dropped
    pub fn init(service_name: &'static str) -> Result<Self> {
        let guard = Self;
        let env_filter_layer = EnvFilter::builder()
            .with_default_directive(LevelFilter::TRACE.into())
            .from_env_lossy();
        let console_output_layer = tracing_subscriber::fmt::layer().with_filter(env_filter_layer);
        let mut layers = vec![console_output_layer.boxed()];

        let env_filter_layer = EnvFilter::builder()
            .with_default_directive(LevelFilter::TRACE.into())
            .from_env_lossy();

        let exporter = opentelemetry_otlp::new_exporter().tonic();
        let otel_tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(
                trace::config()
                    .with_id_generator(RandomIdGenerator::default()) // TODO (@NickLarsenNZ): Is there a more appropriate ID generator?
                    .with_resource(Resource::new(vec![KeyValue::new(
                        resource::SERVICE_NAME,
                        service_name,
                    )])),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .context(InstallOtelExporterSnafu)?;

        layers.push(
            tracing_opentelemetry::layer()
                .with_tracer(otel_tracer)
                .with_filter(env_filter_layer)
                .boxed(),
        );

        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layers))
            .context(SetGlobalDefaultSubscriberSnafu)?;

        opentelemetry::global::set_text_map_propagator(
            // NOTE (@NickLarsenNZ): There are various propagators. Eg: TraceContextPropagator
            // standardises HTTP headers to propagate trace-id, parent-id, etc... while the
            // BaggagePropagator sets a "baggage" header with the value being key=value pairs. There
            // are other kinds too. There is also B3 and Jaeger, and some legacy stuff like OT Trace
            // and OpenCensus.
            // See: https://opentelemetry.io/docs/specs/otel/context/api-propagators/
            TraceContextPropagator::new(),
        );

        Ok(guard)
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        // NOTE (@NickLarsenNZ): This might eventually be replaced with something like SdkMeterProvider::shutdown(&self)
        // see: https://github.com/open-telemetry/opentelemetry-rust/pull/1412/files#r1409608679
        opentelemetry::global::shutdown_tracer_provider();
    }
}
