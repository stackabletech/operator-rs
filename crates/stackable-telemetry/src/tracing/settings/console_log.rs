//! Console Log Subscriber Settings.

use std::ops::Deref;

use tracing::level_filters::LevelFilter;

use super::{Settings, SettingsBuilder};

/// Configure specific settings for the Console Log subscriber.
#[derive(Debug, Default, PartialEq)]
pub enum ConsoleLogSettings {
    #[default]
    Disabled,

    Enabled {
        /// Common subscriber settings that apply to the Console Log Subscriber.
        common_settings: Settings,

        /// Console Subscriber log event output format.
        log_format: Format,
    },
}

// impl Deref for ConsoleLogSettings {
//     type Target = Settings;

//     fn deref(&self) -> &Self::Target {
//         &self.common_settings
//     }
// }

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
        ConsoleLogSettings::Enabled {
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

impl From<Settings> for ConsoleLogSettings {
    fn from(common_settings: Settings) -> Self {
        ConsoleLogSettings::Enabled {
            common_settings,
            log_format: Default::default(),
        }
    }
}

impl<T> From<Option<T>> for ConsoleLogSettings
where
    T: Into<ConsoleLogSettings>,
{
    fn from(settings: Option<T>) -> Self {
        match settings {
            Some(settings) => settings.into(),
            None => ConsoleLogSettings::default(),
        }
    }
}

// impl From<(&'static str, LevelFilter)> for ConsoleLogSettings {
//     fn from(value: (&'static str, LevelFilter)) -> Self {
//         Self {
//             common_settings: Settings {
//                 environment_variable: value.0,
//                 default_level: value.1,
//                 enabled: true,
//             },
//             ..Default::default()
//         }
//     }
// }

// impl From<(&'static str, LevelFilter, bool)> for ConsoleLogSettings {
//     fn from(value: (&'static str, LevelFilter, bool)) -> Self {
//         Self {
//             common_settings: Settings {
//                 environment_variable: value.0,
//                 default_level: value.1,
//                 enabled: value.2,
//             },
//             ..Default::default()
//         }
//     }
// }

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = ConsoleLogSettings::Enabled {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
            },
            log_format: Format::Plain,
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .console_log_settings_builder()
            .with_log_format(Format::Plain)
            // color
            .build();

        assert_eq!(expected, result);
    }
}
