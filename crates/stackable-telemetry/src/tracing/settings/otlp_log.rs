use tracing::level_filters::LevelFilter;

use super::{Build, CommonSettings, Settings, SettingsBuilder, SettingsDouble, SettingsTriple};

#[derive(Debug, Default, PartialEq)]
pub struct OtlpLogSettings {
    pub common_settings: Settings,
}

pub struct OtlpLogSettingsBuilder {
    pub(crate) common_settings: Settings,
}

impl OtlpLogSettingsBuilder {
    pub fn build(self) -> OtlpLogSettings {
        self.into()
    }
}

impl From<SettingsBuilder> for OtlpLogSettingsBuilder {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.into(),
        }
    }
}

impl From<OtlpLogSettingsBuilder> for OtlpLogSettings {
    fn from(value: OtlpLogSettingsBuilder) -> Self {
        Self {
            common_settings: value.common_settings,
        }
    }
}

impl Build<OtlpLogSettings> for SettingsBuilder {
    fn build(self) -> OtlpLogSettings {
        OtlpLogSettings {
            common_settings: self.into(),
            ..Default::default()
        }
    }
}

impl CommonSettings for OtlpLogSettings {
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

impl From<SettingsDouble> for OtlpLogSettings {
    fn from(value: SettingsDouble) -> Self {
        Self {
            common_settings: value.into(),
            ..Default::default()
        }
    }
}

impl From<SettingsTriple> for OtlpLogSettings {
    fn from(value: SettingsTriple) -> Self {
        Self {
            common_settings: value.into(),
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
        let expected = OtlpLogSettings {
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
            .otlp_log_settings_builder()
            .build();

        assert_eq!(expected, result);
    }
}
