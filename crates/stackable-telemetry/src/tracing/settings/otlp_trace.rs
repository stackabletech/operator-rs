//! OTLP Trace Subscriber Settings.

use tracing::level_filters::LevelFilter;

use super::{Settings, SettingsBuilder, SettingsToggle};

/// Configure specific settings for the OpenTelemetry trace subscriber.
#[derive(Debug, Default, PartialEq)]
pub enum OtlpTraceSettings {
    /// OpenTelemetry trace subscriber disabled.
    #[default]
    Disabled,

    /// OpenTelemetry trace subscriber enabled.
    Enabled {
        /// Common subscriber settings that apply to the OpenTelemetry trace subscriber.
        common_settings: Settings,
    },
}

impl SettingsToggle for OtlpTraceSettings {
    fn is_enabled(&self) -> bool {
        match self {
            OtlpTraceSettings::Disabled => false,
            OtlpTraceSettings::Enabled { .. } => true,
        }
    }
}

/// For building [`OtlpTraceSettings`].
///
/// <div class="warning">
///
/// Do not use directly, instead use the [`Settings::builder`] associated function.
///
/// </div>
pub struct OtlpTraceSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpTraceSettingsBuilder {
    /// Consumes `self` and builds [`OtlpTraceSettings`].
    pub fn build(self) -> OtlpTraceSettings {
        OtlpTraceSettings::Enabled {
            common_settings: self.common_settings,
        }
    }
}

/// This implementation is used to turn the common settings builder into the OTLP trace specific
/// settings builder via the [`SettingsBuilder::otlp_trace_settings_builder`] function.
impl From<SettingsBuilder> for OtlpTraceSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.build(),
        }
    }
}

impl From<Settings> for OtlpTraceSettings {
    fn from(common_settings: Settings) -> Self {
        Self::Enabled { common_settings }
    }
}

impl<T> From<Option<T>> for OtlpTraceSettings
where
    T: Into<OtlpTraceSettings>,
{
    fn from(settings: Option<T>) -> Self {
        match settings {
            Some(settings) => settings.into(),
            None => OtlpTraceSettings::default(),
        }
    }
}

impl From<(&'static str, LevelFilter)> for OtlpTraceSettings {
    fn from(value: (&'static str, LevelFilter)) -> Self {
        Self::Enabled {
            common_settings: Settings {
                environment_variable: value.0,
                default_level: value.1,
            },
        }
    }
}

impl From<(&'static str, LevelFilter, bool)> for OtlpTraceSettings {
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
        let expected = OtlpTraceSettings::Enabled {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
            },
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .otlp_trace_settings_builder()
            .build();

        assert_eq!(expected, result);
    }
}
