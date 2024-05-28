pub use stackable_versioned_macros::*;

pub trait AsVersionStr {
    const VERSION: &'static str;

    fn as_version_str(&self) -> &'static str {
        Self::VERSION
    }
}
