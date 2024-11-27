//! Console Log Subscriber Settings.

use tracing::level_filters::LevelFilter;

use super::{Build, CommonSettings, Settings, SettingsBuilder};

/// Configure specific settings for the Console Log subscriber.
#[derive(Debug, Default, PartialEq)]
pub struct ConsoleLogSettings {
    /// Common subscriber settings that apply to the Console Log Subscriber.
    pub common_settings: Settings,

    /// Console Subscriber log event output format.
    pub log_format: Format,
}

/// Console Subscriber log event output formats.
///
/// Currently, only [Plain][Format::Plain] is supported.
#[derive(Debug, Default, PartialEq)]
pub enum Format {
    /// Use the plain unstructured log output.
    ///
    /// ANSI color output is enabled by default, but can be disabled at runtime by
    /// setting `NO_COLOR` to a non-empty value.
    ///
    /// See: [`Layer::with_ansi`][tracing_subscriber::fmt::Layer::with_ansi].
    #[default]
    Plain,
    // Json { pretty: bool },
    // LogFmt,
}

/// For building [`ConsoleLogSettings`].
///
/// <div class="warning">
/// Do not use directly, instead use the [`Settings::builder`] associated function.
/// </div>
pub struct ConsoleLogSettingsBuilder {
    pub(crate) common_settings: Settings,
    pub(crate) log_format: Format,
}

impl ConsoleLogSettingsBuilder {
    pub fn with_log_format(mut self, format: Format) -> Self {
        self.log_format = format;
        self
    }

    pub fn build(self) -> ConsoleLogSettings {
        self.into()
    }
}

impl From<ConsoleLogSettingsBuilder> for ConsoleLogSettings {
    fn from(value: ConsoleLogSettingsBuilder) -> Self {
        Self {
            common_settings: value.common_settings,
            log_format: value.log_format,
        }
    }
}

impl From<SettingsBuilder> for ConsoleLogSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.into(),
            log_format: Format::default(),
        }
    }
}

impl Build<ConsoleLogSettings> for SettingsBuilder {
    fn build(self) -> ConsoleLogSettings {
        ConsoleLogSettings {
            common_settings: self.into(),
            ..Default::default()
        }
    }
}

impl CommonSettings for ConsoleLogSettings {
    fn environment_variable(&self) -> &'static str {
        self.common_settings.environment_variable
    }

    fn default_level(&self) -> LevelFilter {
        self.common_settings.default_level
    }

    fn enabled(&self) -> bool {
        self.common_settings.enabled
    }
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = ConsoleLogSettings {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
                enabled: true,
            },
            log_format: Format::Plain,
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .enabled(true)
            .console_log_settings_builder()
            .with_log_format(Format::Plain)
            // color
            .build();

        assert_eq!(expected, result);
    }
}
