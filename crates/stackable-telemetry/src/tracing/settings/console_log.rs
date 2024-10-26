use tracing::level_filters::LevelFilter;

use super::{Build, CommonSettings, Settings, SettingsBuilder};

#[derive(Debug, Default, PartialEq)]
pub struct ConsoleLogSettings {
    pub common_settings: Settings,
    pub log_format: Format,
}

#[derive(Debug, Default, PartialEq)]
pub enum Format {
    #[default]
    Plain,
    // Json { pretty: bool },
    // LogFmt,
}

pub struct ConsoleLogSettingsBuilder {
    pub(crate) common_settings: Settings,
    pub(crate) log_format: Format,
}

impl ConsoleLogSettingsBuilder {
    pub fn log_format(mut self, format: Format) -> Self {
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
            .environment_variable("hello")
            .default_level(LevelFilter::DEBUG)
            .enabled(true)
            .console_log_settings_builder()
            .log_format(Format::Plain)
            // color
            .build();

        assert_eq!(expected, result);
    }
}
