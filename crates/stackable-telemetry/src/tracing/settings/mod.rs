use tracing::level_filters::LevelFilter;

pub mod console_log;
pub use console_log::*;

#[derive(Debug, PartialEq)]
pub struct Settings {
    pub environment_variable: &'static str,

    pub enabled: bool,

    pub default_level: LevelFilter,
}

impl Settings {
    pub fn builder() -> SettingsBuilder {
        SettingsBuilder::default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        SettingsBuilder::default().into()
    }
}

pub struct SettingsBuilder {
    environment_variable: &'static str,
    enabled: bool,
    default_level: LevelFilter,
}

pub trait Build<T> {
    fn build(self) -> T;
}

impl Build<Settings> for SettingsBuilder {
    fn build(self) -> Settings {
        self.into()
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

impl SettingsBuilder {
    pub fn env_var(mut self, name: &'static str) -> Self {
        self.environment_variable = name;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn default_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.default_level = level.into();
        self
    }

    // consider making generic build functions for each type of settings
    // pub fn build(self) -> Settings {
    //     self.into()
    // }

    pub fn console_builder(self) -> ConsoleLogSettingsBuilder {
        self.into()
    }

    // pub fn xxx_builder(self) -> XxxSettingsBuilder {
    //     self.into()
    // }
}

impl Default for SettingsBuilder {
    fn default() -> Self {
        Self {
            environment_variable: "RUST_LOG",
            enabled: false,
            default_level: LevelFilter::OFF,
        }
    }
}

impl From<SettingsBuilder> for Settings {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            environment_variable: value.environment_variable,
            enabled: value.enabled,
            default_level: value.default_level,
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

#[derive(Debug, Default, PartialEq)]
pub enum Format {
    #[default]
    Plain,
    // Json { pretty: bool },
    // LogFmt,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn builds_console_settings() {
        let expected = ConsoleLogSettings {
            common_settings: Settings {
                environment_variable: "hello",
                enabled: true,
                default_level: LevelFilter::DEBUG,
            },
            log_format: Format::Plain,
        };
        let result: ConsoleLogSettings = Settings::builder()
            .enabled(true)
            .env_var("hello")
            .default_level(LevelFilter::DEBUG)
            .console_builder()
            .log_format(Format::Plain)
            // color
            .build();

        assert_eq!(expected, result);
    }
}
