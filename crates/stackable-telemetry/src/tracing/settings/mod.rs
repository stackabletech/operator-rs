//! Subscriber settings.

use tracing::level_filters::LevelFilter;

pub mod console_log;
pub use console_log::*;

pub mod otlp_log;
pub use otlp_log::*;

pub mod otlp_trace;
pub use otlp_trace::*;

/// Simplifies access common settings from subscriber specific settings.
pub trait CommonSettings {
    /// Access to the [`Settings::environment_variable`] field.
    fn environment_variable(&self) -> &'static str;

    /// Access to the [`Settings::default_level`] field.
    fn default_level(&self) -> LevelFilter;

    /// Access to the [`Settings::enabled`] field.
    fn enabled(&self) -> bool;
}

/// General settings that apply to any subscriber.
#[derive(Debug, PartialEq)]
pub struct Settings {
    /// The environment variable used to set the [`LevelFilter`].
    ///
    /// When the environment variable is set, it will override what is set by
    /// [`Self::default_level`].
    pub environment_variable: &'static str,

    /// The [`LevelFilter`] to fallback to if [`Self::environment_variable`] has
    /// not been set.
    pub default_level: LevelFilter,

    /// Whether or not the subscriber is enabled.
    ///
    /// When set to `true`, the [`tracing::Subscriber`] will be added to the
    /// [`tracing_subscriber::Layer`] list.
    pub enabled: bool,
}

impl Settings {
    /// Builder methods to override defaults.
    pub fn builder() -> SettingsBuilder {
        SettingsBuilder::default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        SettingsBuilder::default().into()
    }
}

/// For building [`Settings`].
pub struct SettingsBuilder {
    environment_variable: &'static str,
    enabled: bool,
    default_level: LevelFilter,
}

/// Finalizer to be implemented on builders.
pub trait Build<T> {
    /// Finalize settings.
    fn build(self) -> T;
}

impl Build<Settings> for SettingsBuilder {
    fn build(self) -> Settings {
        self.into()
    }
}

impl SettingsBuilder {
    /// Set the environment variable used for overriding the [`Settings::default_level`].
    ///
    /// Defaults to `RUST_LOG`.
    // TODO (@NickLarsenNZ): set a constant for the default environment variable.
    pub fn with_environment_variable(mut self, name: &'static str) -> Self {
        self.environment_variable = name;
        self
    }

    /// Set the default [`LevelFilter`].
    ///
    /// Defaults to [`LevelFilter::OFF`].
    // TODO (@NickLarsenNZ): set a constant for the default level.
    pub fn with_default_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.default_level = level.into();
        self
    }

    /// Enable or disable the [`tracing::Subscriber`].
    ///
    /// Defaults to `false`.
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

    /// Set specific [`ConsoleLogSettings`].
    pub fn console_log_settings_builder(self) -> ConsoleLogSettingsBuilder {
        self.into()
    }

    /// Set specific [`OtlpLogSettings`].
    pub fn otlp_log_settings_builder(self) -> OtlpLogSettingsBuilder {
        self.into()
    }

    /// Set specific [`OtlpTraceSettings`].
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
