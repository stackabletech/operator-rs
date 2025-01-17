//! OTLP Log Subscriber Settings.

use std::ops::Deref;

use tracing::level_filters::LevelFilter;

use super::{Settings, SettingsBuilder};

#[derive(Debug, Default, PartialEq)]
pub struct OtlpLogSettings {
    pub common_settings: Settings,
}

impl Deref for OtlpLogSettings {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.common_settings
    }
}

pub struct OtlpLogSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpLogSettingsBuilder {
    pub fn build(self) -> OtlpLogSettings {
        OtlpLogSettings {
            common_settings: self.common_settings,
        }
    }
}

/// This implementation is used to turn the common settings builder into the OTLP log specific
/// settings builder via the [`SettingsBuilder::otlp_log_settings_builder`] function.
impl From<SettingsBuilder> for OtlpLogSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.build(),
        }
    }
}

impl From<Settings> for OtlpLogSettings {
    fn from(common_settings: Settings) -> Self {
        Self { common_settings }
    }
}

impl From<(&'static str, LevelFilter)> for OtlpLogSettings {
    fn from(value: (&'static str, LevelFilter)) -> Self {
        Self {
            common_settings: Settings {
                environment_variable: value.0,
                default_level: value.1,
                enabled: true,
            },
        }
    }
}

impl From<(&'static str, LevelFilter, bool)> for OtlpLogSettings {
    fn from(value: (&'static str, LevelFilter, bool)) -> Self {
        Self {
            common_settings: Settings {
                environment_variable: value.0,
                default_level: value.1,
                enabled: value.2,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = OtlpLogSettings {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
                enabled: true,
            },
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .enabled(true)
            .otlp_log_settings_builder()
            .build();

        assert_eq!(expected, result);
    }
}
