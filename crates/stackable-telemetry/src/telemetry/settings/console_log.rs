//! Console Log Subscriber Settings.

use std::ops::Deref;

use super::{Build, Settings, SettingsBuilder};

/// Configure specific settings for the Console Log subscriber.
#[derive(Debug, Default, PartialEq)]
pub struct ConsoleLogSettings {
    /// Common subscriber settings that apply to the Console Log Subscriber.
    pub common_settings: Settings,

    /// Console Subscriber log event output format.
    pub log_format: Format,
}

impl Deref for ConsoleLogSettings {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.common_settings
    }
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
        ConsoleLogSettings {
            common_settings: self.common_settings,
            log_format: self.log_format,
        }
    }
}

/// This implementation is used to turn the common settings builder into the console log specific
/// settings builder via the [`SettingsBuilder::console_log_settings_builder`] function.
impl From<SettingsBuilder> for ConsoleLogSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.build(),
            log_format: Format::default(),
        }
    }
}

/// This implementation is used to build console log settings from common settings without
/// specifying console log specific settings.
impl Build<ConsoleLogSettings> for SettingsBuilder {
    fn build(self) -> ConsoleLogSettings {
        ConsoleLogSettings {
            common_settings: self.build(),
            ..Default::default()
        }
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
