//! OTLP Metric Exporter Settings.

use super::SettingsToggle;

/// Configure the OpenTelemetry metric exporter.
///
/// Unlike the log and trace subscribers, the metric exporter has no level filter: every instrument
/// recorded through the global meter provider is exported.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum OtlpMetricSettings {
    /// OpenTelemetry metric exporter disabled.
    #[default]
    Disabled,

    /// OpenTelemetry metric exporter enabled.
    Enabled,
}

impl SettingsToggle for OtlpMetricSettings {
    fn is_enabled(&self) -> bool {
        match self {
            Self::Disabled => false,
            Self::Enabled => true,
        }
    }
}

impl From<bool> for OtlpMetricSettings {
    fn from(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

impl<T> From<Option<T>> for OtlpMetricSettings
where
    T: Into<Self>,
{
    fn from(settings: Option<T>) -> Self {
        match settings {
            Some(settings) => settings.into(),
            None => Self::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_bool() {
        assert_eq!(OtlpMetricSettings::from(true), OtlpMetricSettings::Enabled);
        assert_eq!(
            OtlpMetricSettings::from(false),
            OtlpMetricSettings::Disabled
        );
        assert!(OtlpMetricSettings::default().is_disabled());
    }

    #[test]
    fn from_option() {
        assert!(OtlpMetricSettings::from(Some(true)).is_enabled());
        assert!(OtlpMetricSettings::from(Some(false)).is_disabled());
        assert!(OtlpMetricSettings::from(None::<bool>).is_disabled());
    }
}
