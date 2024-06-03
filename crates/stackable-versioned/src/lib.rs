//! This crate enables versioning of structs (and enums in the future). It
//! currently supports Kubernetes API versions while declaring versions on a
//! data type. This will be extended to support SemVer versions, as well as
//! custom version formats in the future.
//!
//! ## Basic Usage
//!
//! ```
//! #[versioned(
//!     version(name = "v1alpha1"),
//!     version(name = "v1beta1"),
//!     version(name = "v1"),
//!     version(name = "v2"),
//!     version(name = "v3")
//! )]
//! struct Foo {
//!     /// My docs
//!     #[versioned(
//!         added(since = "v1alpha1"),
//!         renamed(since = "v1beta1", from = "gau"),
//!         deprecated(since = "v2", note = "not empty")
//!     )]
//!     deprecated_bar: usize,
//!     baz: bool,
//! }
//! ```

pub use stackable_versioned_macros::*;

pub trait AsVersionStr {
    const VERSION: &'static str;

    fn as_version_str(&self) -> &'static str {
        Self::VERSION
    }
}
