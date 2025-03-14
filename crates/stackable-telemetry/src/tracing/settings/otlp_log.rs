//! OTLP Log Subscriber Settings.

use tracing::level_filters::LevelFilter;

use super::{Settings, SettingsBuilder, SettingsToggle};

/// Configure specific settings for the OpenTelemetry log subscriber.
#[derive(Debug, Default, PartialEq)]
pub enum OtlpLogSettings {
    /// OpenTelemetry log subscriber disabled.
    #[default]
    Disabled,

    /// OpenTelemetry log subscriber enabled.
    Enabled {
        /// Common subscriber settings that apply to the OpenTelemetry log subscriber.
        common_settings: Settings,
    },
}

impl SettingsToggle for OtlpLogSettings {
    fn is_enabled(&self) -> bool {
        match self {
            OtlpLogSettings::Disabled => false,
            OtlpLogSettings::Enabled { .. } => true,
        }
    }
}

/// For building [`OtlpLogSettings`].
///
/// <div class="warning">
///
/// Do not use directly, instead use the [`Settings::builder`] associated function.
///
/// </div>
pub struct OtlpLogSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpLogSettingsBuilder {
    /// Consumes `self` and builds [`OtlpLogSettings`].
    pub fn build(self) -> OtlpLogSettings {
        OtlpLogSettings::Enabled {
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
        Self::Enabled { common_settings }
    }
}

impl<T> From<Option<T>> for OtlpLogSettings
where
    T: Into<OtlpLogSettings>,
{
    fn from(settings: Option<T>) -> Self {
        match settings {
            Some(settings) => settings.into(),
            None => OtlpLogSettings::default(),
        }
    }
}

impl From<(&'static str, LevelFilter)> for OtlpLogSettings {
    fn from(value: (&'static str, LevelFilter)) -> Self {
        Self::Enabled {
            common_settings: Settings {
                environment_variable: value.0,
                default_level: value.1,
            },
        }
    }
}

impl From<(&'static str, LevelFilter, bool)> for OtlpLogSettings {
    fn from(value: (&'static str, LevelFilter, bool)) -> Self {
        match value.2 {
            true => Self::Enabled {
                common_settings: Settings {
                    environment_variable: value.0,
                    default_level: value.1,
                },
            },
            false => Self::Disabled,
        }
    }
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = OtlpLogSettings::Enabled {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
            },
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .otlp_log_settings_builder()
            .build();

        assert_eq!(expected, result);
    }
}
