//! OTLP Log Subscriber Settings.

use std::ops::Deref;

use super::{Build, Settings, SettingsBuilder};

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

/// This implementation is used to build OTLP log settings from common settings without
/// specifying OTLP log specific settings.
impl Build<OtlpLogSettings> for SettingsBuilder {
    fn build(self) -> OtlpLogSettings {
        OtlpLogSettings {
            common_settings: self.build(),
            // ..Default::default()
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
