use super::{Format, Settings};

#[derive(Debug, Default, PartialEq)]
pub struct ConsoleLogSettings {
    pub common_settings: Settings,
    pub log_format: Format,
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
