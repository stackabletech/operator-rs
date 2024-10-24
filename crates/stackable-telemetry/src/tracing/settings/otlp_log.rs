use super::Settings;

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

impl From<OtlpLogSettingsBuilder> for OtlpLogSettings {
    fn from(value: OtlpLogSettingsBuilder) -> Self {
        Self {
            common_settings: value.common_settings,
        }
    }
}
