//! File Log Subscriber Settings.

use std::{ops::Deref, path::PathBuf};

use super::{Build, Settings, SettingsBuilder};

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

/// This trait is only used for the typestate builder and cannot be implemented
/// outside of this crate.
///
/// The only reason it has pub visibility is because it needs to be at least as
/// visible as the types that use it.
#[doc(hidden)]
pub trait BuilderState: private::Sealed {}

/// This private module holds the [`Sealed`][1] trait that is used by the
/// [`BuilderState`], so that it cannot be implemented outside of this crate.
///
/// We impl Sealed for any types that will use the trait that we want to
/// restrict impls on. In this case, the [`BuilderState`] trait.
///
/// [1]: private::Sealed
#[doc(hidden)]
mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for builder_state::PreLogDir {}
    impl Sealed for builder_state::Config {}
}

/// This module holds the possible states that the builder is in.
///
/// Each state will implement [`BuilderState`] (with no methods), and the
/// Builder struct ([`FileLogSettingsBuilder`][1]) itself will be implemented with
/// each state as a generic parameter.
/// This allows only the methods to be called when the builder is in the
/// applicable state.
///
/// [1]: super::FileLogSettingsBuilder
#[doc(hidden)]
pub mod builder_state {
    /// The initial state, before the log directory path is set.
    #[derive(Default)]
    pub struct PreLogDir;

    /// The state that allows you to configure the [`FileLogSettings`][1].
    ///
    /// [1]: super::FileLogSettings
    #[derive(Default)]
    pub struct Config;
}

// Make the states usable
#[doc(hidden)]
impl BuilderState for builder_state::PreLogDir {}

#[doc(hidden)]
impl BuilderState for builder_state::Config {}

/// For building [`FileLogSettings`].
///
/// <div class="warning">
/// Do not use directly, instead use the [`Settings::builder`] associated function.
/// </div>
pub struct FileLogSettingsBuilder<S: BuilderState> {
    pub(crate) common_settings: Settings,
    pub(crate) file_log_dir: Option<PathBuf>,

    /// Allow the generic to be used (needed for impls).
    _marker: std::marker::PhantomData<S>,
}

impl FileLogSettingsBuilder<builder_state::PreLogDir> {
    /// Set the directory for log files.
    ///
    /// A directory is required for using the File Log subscriber.
    pub fn with_file_log_dir(self, path: String) -> FileLogSettingsBuilder<builder_state::Config> {
        FileLogSettingsBuilder {
            common_settings: self.common_settings,
            file_log_dir: Some(PathBuf::from(path)),
            _marker: std::marker::PhantomData,
        }
    }
}

impl FileLogSettingsBuilder<builder_state::Config> {
    /// Consumes self and returns a valid [`FileLogSettings`] instance.
    pub fn build(self) -> FileLogSettings {
        FileLogSettings {
            common_settings: self.common_settings,
            file_log_dir: self
                .file_log_dir
                .expect("file_log_dir must be configured at this point"),
        }
    }
}

/// This implementation is used to turn the common settings builder into the file log specific
/// settings builder via the [`SettingsBuilder::file_log_settings_builder`] function.
impl From<SettingsBuilder> for FileLogSettingsBuilder<builder_state::PreLogDir> {
    fn from(value: SettingsBuilder) -> Self {
        Self {
            common_settings: value.build(),
            file_log_dir: None,
            _marker: std::marker::PhantomData,
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
            .file_log_settings_builder()
            .with_file_log_dir(String::from("/logs"))
            .build();

        assert_eq!(expected, result);
    }
}
