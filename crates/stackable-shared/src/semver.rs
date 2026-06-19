use std::sync::LazyLock;

/// A [`semver::Version`] containing `0.0.0-dev`.
pub static ZERO_ZERO_ZERO_DEV: LazyLock<semver::Version> = LazyLock::new(|| semver::Version {
    major: 0,
    minor: 0,
    patch: 0,
    pre: semver::Prerelease::new("dev").expect("static prerelease must be valid"),
    build: semver::BuildMetadata::EMPTY,
});

pub trait VersionExt {
    /// Returns the floating version as a [`String`], eg. `26.7.0` -> `26.7`
    fn floating(&self) -> String;
}

impl VersionExt for ::semver::Version {
    fn floating(&self) -> String {
        format!("{major}.{minor}", major = self.major, minor = self.minor)
    }
}
