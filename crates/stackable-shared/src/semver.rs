pub trait VersionExt {
    /// Returns the floating version as a [`String`], eg. `26.7.0` -> `26.7`
    fn floating(&self) -> String;
}

impl VersionExt for ::semver::Version {
    fn floating(&self) -> String {
        format!("{major}.{minor}", major = self.major, minor = self.minor)
    }
}
