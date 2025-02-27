//! File Log Subscriber Settings.

use std::{ops::Deref, path::PathBuf};

use super::Settings;

/// Configure specific settings for the File Log subscriber.
#[derive(Debug, Default, PartialEq)]
pub struct FileLogSettings {
    /// Common subscriber settings that apply to the File Log Subscriber.
    pub common_settings: Settings,

    /// Path to directory for log files.
    pub file_log_dir: PathBuf,
}

impl Deref for FileLogSettings {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.common_settings
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
}

impl FileLogSettingsBuilder {
    /// Consumes self and returns a valid [`FileLogSettings`] instance.
    pub fn build(self) -> FileLogSettings {
        FileLogSettings {
            common_settings: self.common_settings,
            file_log_dir: self.file_log_dir,
        }
    }
}

#[cfg(test)]
mod test {
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[test]
    fn builds_settings() {
        let expected = FileLogSettings {
            common_settings: Settings {
                environment_variable: "hello",
                default_level: LevelFilter::DEBUG,
                enabled: true,
            },
            file_log_dir: PathBuf::from("/logs"),
        };
        let result = Settings::builder()
            .with_environment_variable("hello")
            .with_default_level(LevelFilter::DEBUG)
            .enabled(true)
            .file_log_settings_builder(PathBuf::from("/logs"))
            .build();

        assert_eq!(expected, result);
    }
}
