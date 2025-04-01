//! File Log Subscriber Settings.

use std::path::PathBuf;

/// Re-export to save the end crate
pub use tracing_appender::rolling::Rotation;

use super::{Settings, SettingsToggle};

/// Configure specific settings for the File Log subscriber.
#[derive(Debug, Default, PartialEq)]
pub enum FileLogSettings {
    /// File Log subscriber disabled.
    #[default]
    Disabled,

    /// File Log subscriber enabled.
    Enabled {
        /// Common subscriber settings that apply to the File Log Subscriber.
        common_settings: Settings,

        /// Path to directory for log files.
        file_log_dir: PathBuf,

        /// Log rotation frequency.
        rotation_period: Rotation,
    },
}

impl SettingsToggle for FileLogSettings {
    fn is_enabled(&self) -> bool {
        match self {
            FileLogSettings::Disabled => false,
            FileLogSettings::Enabled { .. } => true,
        }
    }
}

/// For building [`FileLogSettings`].
///
/// <div class="warning">
/// Do not use directly, instead use the [`Settings::builder`] associated function.
/// </div>
pub struct FileLogSettingsBuilder {
    pub(crate) common_settings: Settings,
    pub(crate) file_log_dir: PathBuf,
    pub(crate) rotation_period: Rotation,
}

impl FileLogSettingsBuilder {
    /// Set file rotation period.
    pub fn with_rotation_period(mut self, rotation_period: Rotation) -> Self {
        self.rotation_period = rotation_period;
        self
    }

    /// Consumes self and returns a valid [`FileLogSettings`] instance.
    pub fn build(self) -> FileLogSettings {
        FileLogSettings::Enabled {
            common_settings: self.common_settings,
            file_log_dir: self.file_log_dir,
            rotation_period: self.rotation_period,
        }
    }
}

impl<T> From<Option<T>> for FileLogSettings
where
    T: Into<FileLogSettings>,
{
    fn from(settings: Option<T>) -> Self {
        match settings {
            Some(settings) => settings.into(),
            None => FileLogSettings::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = FileLogSettings::Enabled {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
            },
            file_log_dir: PathBuf::from("/logs"),
            rotation_period: Rotation::HOURLY,
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .file_log_settings_builder(PathBuf::from("/logs"))
            .with_rotation_period(Rotation::HOURLY)
            .build();

        assert_eq!(expected, result);
    }
}
