use super::Settings;

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

impl From<OtlpTraceSettingsBuilder> for OtlpTraceSettings {
    fn from(value: OtlpTraceSettingsBuilder) -> Self {
        Self {
            common_settings: value.common_settings,
        }
    }
}
