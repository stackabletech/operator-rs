use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::time::Duration;

/// Configure [OpenTelemetry][1] related config fields for the operator.
///
/// It contains sub fields to individually configure [metrics][2], [traces][3],
/// and [logs][4]. Additionally, configure global [exporter](ExporterConfig)
/// settings. This is especially useful when all telemetry data is sent to a
/// central collector.
///
/// [1]: https://opentelemetry.io/
/// [2]: https://opentelemetry.io/docs/specs/otel/metrics/
/// [3]: https://opentelemetry.io/docs/specs/otel/trace/
/// [4]: https://opentelemetry.io/docs/specs/otel/logs/
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTelemetryConfig {
    pub metrics: MetricsConfig,
    pub traces: TracesConfig,
    pub logs: LogsConfig,

    /// Global default exporter configuration.
    pub defaults: ExporterConfig,
}

/// Configure OpenTelemetry [metrics][1] settings for the operator.
///
/// [1]: https://opentelemetry.io/docs/specs/otel/metrics/
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsConfig {
    /// Enables the export of metrics.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

/// Configure OpenTelemetry [trace][1] settings for the operator.
///
/// [1]: https://opentelemetry.io/docs/specs/otel/trace/
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracesConfig {
    /// Enables the export of traces.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

/// Configure OpenTelemetry [log][1] settings for the operator.
///
/// [1]: https://opentelemetry.io/docs/specs/otel/logs/
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsConfig {
    /// Enables the export of logs.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

/// Configure OpenTelemetry export settings, like endpoint and timeout.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExporterConfig {
    /// The OLTP endpoint.
    ///
    /// Must be a valid URL, like `https://my.export.corp:4317`.
    pub endpoint: Url,

    /// The timeout when sending data to the collector.
    ///
    /// See <DOCS_BASE_URL_PLACEHOLDER/reference/duration> for more details.
    #[serde(default = "default_exporter_timeout")]
    pub timeout: Duration,
}

const fn default_exporter_timeout() -> Duration {
    Duration::from_secs(2)
}

const fn r#true() -> bool {
    true
}

#[cfg(test)]
mod test {
    use schemars::schema_for;

    use super::*;

    #[test]
    fn json_schema() {
        let schema = schema_for!(OpenTelemetryConfig);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap())
    }
}
