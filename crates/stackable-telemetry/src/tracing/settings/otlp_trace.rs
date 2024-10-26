use tracing::level_filters::LevelFilter;

use super::{Build, CommonSettings, Settings, SettingsBuilder};

#[derive(Debug, Default, PartialEq)]
pub struct OtlpTraceSettings {
    pub common_settings: Settings,
}

pub struct OtlpTraceSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpTraceSettingsBuilder {
    pub fn build(self) -> OtlpTraceSettings {
        self.into()
    }
}

impl From<SettingsBuilder> for OtlpTraceSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.into(),
        }
    }
}

impl From<OtlpTraceSettingsBuilder> for OtlpTraceSettings {
    fn from(value: OtlpTraceSettingsBuilder) -> Self {
        Self {
            common_settings: value.common_settings,
        }
    }
}

impl Build<OtlpTraceSettings> for SettingsBuilder {
    fn build(self) -> OtlpTraceSettings {
        OtlpTraceSettings {
            common_settings: self.into(),
            ..Default::default()
        }
    }
}

impl CommonSettings for OtlpTraceSettings {
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
        let expected = OtlpTraceSettings {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
                enabled: true,
            },
        };
        let result = Settings::builder()
            .environment_variable("hello")
            .default_level(LevelFilter::DEBUG)
            .enabled(true)
            .otlp_trace_settings_builder()
            .build();

        assert_eq!(expected, result);
    }
}
