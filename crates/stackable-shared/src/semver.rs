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

    /// Returns whether the version is `0.0.0`, independent of pre-release information and build
    /// metadata.
    fn is_0_0_0(&self) -> bool;

    /// Returns whether the version is `0.0.0-dev`, independent of build metadata.
    fn is_0_0_0_dev(&self) -> bool;

    /// Returns whether the version is `0.0.0-pr*`, independent of build metadata.
    fn is_0_0_0_pr(&self) -> bool;

    /// Returns whether the version is considered floating.
    ///
    /// Currently, `0.0.0-dev` and `0.0.0-pr*` versions are considered floating.
    fn is_floating(&self) -> bool;
}

impl VersionExt for ::semver::Version {
    fn floating(&self) -> String {
        if self.is_floating() {
            self.to_string()
        } else {
            format!("{major}.{minor}", major = self.major, minor = self.minor)
        }
    }

    fn is_0_0_0(&self) -> bool {
        self.major == 0 && self.minor == 0 && self.patch == 0
    }

    fn is_0_0_0_dev(&self) -> bool {
        self.is_0_0_0() && self.pre.as_str() == "dev"
    }

    fn is_0_0_0_pr(&self) -> bool {
        self.is_0_0_0() && self.pre.starts_with("pr")
    }

    fn is_floating(&self) -> bool {
        self.is_0_0_0_dev() || self.is_0_0_0_pr()
    }
}
