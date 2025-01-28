//! OTLP Trace Subscriber Settings.

use std::ops::Deref;

use tracing::level_filters::LevelFilter;

use super::{Settings, SettingsBuilder};

#[derive(Debug, Default, PartialEq)]
pub struct OtlpTraceSettings {
    pub common_settings: Settings,
}

impl Deref for OtlpTraceSettings {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.common_settings
    }
}

pub struct OtlpTraceSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpTraceSettingsBuilder {
    pub fn build(self) -> OtlpTraceSettings {
        OtlpTraceSettings {
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
        Self { common_settings }
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

// impl From<(&'static str, LevelFilter)> for OtlpTraceSettings {
//     fn from(value: (&'static str, LevelFilter)) -> Self {
//         Self {
//             common_settings: Settings {
//                 environment_variable: value.0,
//                 default_level: value.1,
//                 enabled: true,
//             },
//         }
//     }
// }

// impl From<(&'static str, LevelFilter, bool)> for OtlpTraceSettings {
//     fn from(value: (&'static str, LevelFilter, bool)) -> Self {
//         Self {
//             common_settings: Settings {
//                 environment_variable: value.0,
//                 default_level: value.1,
//                 enabled: value.2,
//             },
//         }
//     }
// }

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = OtlpTraceSettings {
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
