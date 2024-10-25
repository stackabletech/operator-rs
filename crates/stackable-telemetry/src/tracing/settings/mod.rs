use tracing::level_filters::LevelFilter;

pub mod console_log;
pub use console_log::*;

pub mod otlp_trace;
pub use otlp_trace::*;

#[derive(Debug, PartialEq)]
pub struct Settings {
    pub environment_variable: &'static str,

    pub enabled: bool,

    pub default_level: LevelFilter,
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
    pub fn env_var(mut self, name: &'static str) -> Self {
        self.environment_variable = name;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn default_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.default_level = level.into();
        self
    }

    pub fn console_log_settings_builder(self) -> ConsoleLogSettingsBuilder {
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
            enabled: false,
            default_level: LevelFilter::OFF,
        }
    }
}

impl From<SettingsBuilder> for Settings {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            environment_variable: value.environment_variable,
            enabled: value.enabled,
            default_level: value.default_level,
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
            enabled: true,
            default_level: LevelFilter::DEBUG,
        };
        let result = Settings::builder()
            .enabled(true)
            .env_var("hello")
            .default_level(LevelFilter::DEBUG)
            .build();

        assert_eq!(expected, result);
    }
}
