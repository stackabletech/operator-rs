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
    pub fn environment_variable(mut self, name: &'static str) -> Self {
        self.environment_variable = name;
        self
    }

    pub fn default_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.default_level = level.into();
        self
    }

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

pub(crate) type SettingsDouble = (&'static str, LevelFilter);
pub(crate) type SettingsTriple = (&'static str, LevelFilter, bool);

// for enabling a subscriber in one line with no extra settings
impl From<SettingsDouble> for Settings {
    fn from((environment_variable, default_level): SettingsDouble) -> Self {
        Settings {
            environment_variable,
            default_level,
            enabled: true,
        }
    }
}

// for configuring a subscriber in one line with no extra settings
impl From<SettingsTriple> for Settings {
    fn from((environment_variable, default_level, enabled): SettingsTriple) -> Self {
        Settings {
            environment_variable,
            default_level,
            enabled,
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
            .environment_variable("hello")
            .default_level(LevelFilter::DEBUG)
            .enabled(true)
            .build();

        assert_eq!(expected, result);
    }
}
