//! Subscriber settings.

use std::path::Path;

use tracing::level_filters::LevelFilter;

pub mod console_log;
pub use console_log::*;

pub mod file_log;
pub use file_log::*;

pub mod otlp_log;
pub use otlp_log::*;

pub mod otlp_trace;
pub use otlp_trace::*;

/// Indicate whether a type is enabled or disabled.
pub trait SettingsToggle {
    /// Whether the settings are enabled or not.
    fn is_enabled(&self) -> bool;

    /// The opposite of [SettingsToggle::is_enabled] as a helper.
    fn is_disabled(&self) -> bool {
        !self.is_enabled()
    }
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
}

impl Settings {
    /// Builder methods to override defaults.
    pub fn builder() -> SettingsBuilder {
        SettingsBuilder::default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        SettingsBuilder::default().build()
    }
}

/// For building [`Settings`].
pub struct SettingsBuilder {
    environment_variable: &'static str,
    default_level: LevelFilter,
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

    /// Set specific [`ConsoleLogSettings`].
    pub fn console_log_settings_builder(self) -> ConsoleLogSettingsBuilder {
        self.into()
    }

    /// Set specific [`FileLogSettings`].
    pub fn file_log_settings_builder<P>(self, path: P) -> FileLogSettingsBuilder
    where
        P: AsRef<Path>,
    {
        FileLogSettingsBuilder {
            common_settings: self.build(),
            file_log_dir: path.as_ref().to_path_buf(),
        }
    }

    /// Set specific [`OtlpLogSettings`].
    pub fn otlp_log_settings_builder(self) -> OtlpLogSettingsBuilder {
        self.into()
    }

    /// Set specific [`OtlpTraceSettings`].
    pub fn otlp_trace_settings_builder(self) -> OtlpTraceSettingsBuilder {
        self.into()
    }

    /// Consumes self and constructs valid [`Settings`].
    pub fn build(self) -> Settings {
        Settings {
            environment_variable: self.environment_variable,
            default_level: self.default_level,
        }
    }
}

impl Default for SettingsBuilder {
    fn default() -> Self {
        Self {
            environment_variable: "RUST_LOG",
            default_level: LevelFilter::OFF,
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
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .build();

        assert_eq!(expected, result);
    }
}
