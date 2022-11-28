use std::collections::BTreeMap;
use std::fmt::Display;

use crate::config::merge::Atomic;
use crate::config::{fragment::Fragment, merge::Merge};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(Clone, Debug, Deserialize, Merge, JsonSchema, PartialEq, Serialize),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(
        bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>",),
        rename_all = "camelCase",
    )
)]
pub struct Logging<T>
where
    T: Clone + Display + Ord,
{
    pub enable_vector_agent: bool,
    #[fragment_attrs(serde(default))]
    pub containers: BTreeMap<T, ContainerLogConfig>,
}

impl<T> Default for Logging<T>
where
    T: Clone + Display + Ord,
{
    fn default() -> Self {
        Self {
            enable_vector_agent: Default::default(),
            containers: Default::default(),
        }
    }
}

impl<T> Default for LoggingFragment<T>
where
    T: Clone + Display + Ord,
{
    fn default() -> Self {
        Self {
            enable_vector_agent: Default::default(),
            containers: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        Merge,
        JsonSchema,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct ContainerLogConfig {
    #[fragment_attrs(serde(default))]
    pub loggers: BTreeMap<String, LoggerConfig>,
    #[fragment_attrs(serde(default))]
    pub console: AppenderConfig,
    #[fragment_attrs(serde(default))]
    pub file: AppenderConfig,
}

impl ContainerLogConfig {
    pub const ROOT_LOGGER: &'static str = "ROOT";

    pub fn root_log_level(&self) -> Option<LogLevel> {
        self.loggers
            .get(Self::ROOT_LOGGER)
            .map(|root| root.level.to_owned())
    }
}

#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        Merge,
        JsonSchema,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct LoggerConfig {
    pub level: LogLevel,
}

#[derive(Clone, Debug, Default, Eq, Fragment, JsonSchema, PartialEq)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        Merge,
        JsonSchema,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct AppenderConfig {
    #[fragment_attrs(serde(default))]
    pub level_threshold: LogLevel,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
    FATAL,
    NONE,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::INFO
    }
}

impl Atomic for LogLevel {}

impl LogLevel {
    pub fn to_logback_literal(&self) -> String {
        match self {
            LogLevel::TRACE => "TRACE",
            LogLevel::DEBUG => "DEBUG",
            LogLevel::INFO => "INFO",
            LogLevel::WARN => "WARN",
            LogLevel::ERROR => "ERROR",
            LogLevel::FATAL => "FATAL",
            LogLevel::NONE => "OFF",
        }
        .into()
    }
}

pub fn default_logging<T>() -> LoggingFragment<T>
where
    T: Clone + Display + Ord + strum::IntoEnumIterator,
{
    LoggingFragment {
        enable_vector_agent: Some(true),
        containers: T::iter()
            .map(|container| (container, default_container_log_config()))
            .collect(),
    }
}

pub fn default_container_log_config() -> ContainerLogConfigFragment {
    ContainerLogConfigFragment {
        loggers: [(
            ContainerLogConfig::ROOT_LOGGER.into(),
            LoggerConfigFragment {
                level: Some(LogLevel::INFO),
            },
        )]
        .into(),
        console: AppenderConfigFragment {
            level_threshold: Some(LogLevel::INFO),
        },
        file: AppenderConfigFragment {
            level_threshold: Some(LogLevel::INFO),
        },
    }
}
