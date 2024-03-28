use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::time::Duration;

// NOTE (@Techassi): We might want to rename this to OpenTelemetryConfig to
// avoid confusion about business telemetry.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConfig {
    pub metrics: MetricsConfig,
    pub traces: TracesConfig,
    pub logs: LogsConfig,

    /// Global default exporter configuration.
    pub defaults: ExporterConfig,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsConfig {
    /// Enables the export of metrics.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracesConfig {
    /// Enables the export of traces.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsConfig {
    /// Enables the export of logs.
    #[serde(default = "r#true")]
    pub enabled: bool,

    /// Overides the global exporter config.
    pub exporter: Option<ExporterConfig>,
}

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
        let schema = schema_for!(TelemetryConfig);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap())
    }
}
