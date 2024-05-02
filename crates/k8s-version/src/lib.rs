//! This library provides strongly-typed and validated  Kubernetes API version
//! definitions. Versions consist of three major components: the optional group,
//! the mandatory major version and the optional level. The format can be
//! described by `(<GROUP>/)<VERSION>`, with `<VERSION>` being defined as
//! `v<MAJOR>(beta/alpha<LEVEL>)`.
//!
//! ## Usage
//!
//! Versions can be parsed and validated from [`str`] using Rust's standard
//! [`FromStr`](std::str::FromStr) trait.
//!
//! ```
//! # use std::str::FromStr;
//! use k8s_version::ApiVersion;
//!
//! let api_version = ApiVersion::from_str("extensions/v1beta1")
//!     .expect("valid Kubernetes API version");
//!
//! // Or using .parse()
//! let api_version: ApiVersion = "extensions/v1beta1".parse()
//!     .expect("valid Kubernetes API version");
//! ```
//!
//! Alternatively, they can be constructed programatically using the
//! [`new()`](ApiVersion::new) and [`try_new`](ApiVersion::try_new) function.
//!
//! ```
//! use k8s_version::{ApiVersion, Version, Level};
//!
//! let api_version = ApiVersion::try_new(
//!     Some("extension"),
//!     Version::new(1, Some(Level::Beta(1)))
//! ).expect("valid Kubernetes API version");
//!
//! assert_eq!(api_version.to_string(), "extension/v1beta1")
//! ```

// NOTE (@Techassi): Fixed in https://github.com/la10736/rstest/pull/244 but not
// yet released.
#[cfg(test)]
use rstest_reuse::{self};

mod api_version;
mod group;
mod level;
mod version;

pub use api_version::*;
pub use group::*;
pub use level::*;
pub use version::*;
