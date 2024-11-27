use tracing::level_filters::LevelFilter;

pub mod console_log;
pub use console_log::*;

pub mod otlp_log;
pub use otlp_log::*;

pub mod otlp_trace;
pub use otlp_trace::*;

// this trait is to make it simpler to access common settings from specific settings.
pub trait CommonSettings {
    fn environment_variable(&self) -> &'static str;
    fn default_level(&self) -> LevelFilter;
    fn enabled(&self) -> bool;
}

#[derive(Debug, PartialEq)]
pub struct Settings {
    pub environment_variable: &'static str,

    pub default_level: LevelFilter,

    pub enabled: bool,
}

impl Settings {
    pub fn builder() -> SettingsBuilder {
        SettingsBuilder::default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        SettingsBuilder::default().into()
    }
}

pub struct SettingsBuilder {
    environment_variable: &'static str,
    enabled: bool,
    default_level: LevelFilter,
}

pub trait Build<T> {
    fn build(self) -> T;
}

impl Build<Settings> for SettingsBuilder {
    fn build(self) -> Settings {
        self.into()
    }
}

impl SettingsBuilder {
    pub fn with_environment_variable(mut self, name: &'static str) -> Self {
        self.environment_variable = name;
        self
    }

    pub fn with_default_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.default_level = level.into();
        self
    }

    // TODO (@NickLarsenNZ): Currently this has to be called to enable the
    // subscriber. Eventually it should become optional, and default to on (if
    // settings are supplied). Therefore, the fields in TracingBuilder to hold
    // the subscriber settings should become Option<T> so that the subscriber is
    // disabled when not configured, is enabled when configured, while still
    // controllable through this function. Then this can be renamed to `with_enabled`
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn console_log_settings_builder(self) -> ConsoleLogSettingsBuilder {
        self.into()
    }

    pub fn otlp_log_settings_builder(self) -> OtlpLogSettingsBuilder {
        self.into()
    }

    pub fn otlp_trace_settings_builder(self) -> OtlpTraceSettingsBuilder {
        self.into()
    }
}

impl Default for SettingsBuilder {
    fn default() -> Self {
        Self {
            environment_variable: "RUST_LOG",
            default_level: LevelFilter::OFF,
            enabled: false,
        }
    }
}

impl From<SettingsBuilder> for Settings {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            environment_variable: value.environment_variable,
            default_level: value.default_level,
            enabled: value.enabled,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn builds_settings() {
        let expected = Settings {
            environment_variable: "hello",
            default_level: LevelFilter::DEBUG,
            enabled: true,
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .enabled(true)
            .build();

        assert_eq!(expected, result);
    }
}
