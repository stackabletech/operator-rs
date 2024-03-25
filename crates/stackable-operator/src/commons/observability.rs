use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::time::Duration;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityConfig {
    pub metrics: MetricsConfig,
    pub traces: TracesConfig,
    pub logs: LogsConfig,

    /// Global exporter configuration.
    pub exporter: ExporterConfig,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
}

fn default_metrics_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracesConfig {
    #[serde(default = "default_traces_enabled")]
    pub enabled: bool,
}

fn default_traces_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsConfig {
    #[serde(default = "default_logs_enabled")]
    pub enabled: bool,
}

fn default_logs_enabled() -> bool {
    true
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
    pub timeout: Duration,
}
