//! This library provides strongly-typed and validated Kubernetes API version
//! definitions. Versions consist of three major components: the optional group,
//! the mandatory major version and the optional level. The format can be
//! described by `(<GROUP>/)<VERSION>`, with `<VERSION>` being defined as
//! `v<MAJOR>(alpha<LEVEL>|beta<LEVEL>)`.
//!
//! ## Usage
//!
//! ### Parsing from [`str`]
//!
//! Versions can be parsed and validated from [`str`] using Rust's standard
//! [`FromStr`](std::str::FromStr) trait.
//!
//! ```
//! # use std::str::FromStr;
//! use k8s_version::ApiVersion;
//!
//! let api_version = ApiVersion::from_str("extensions/v1beta1").unwrap();
//!
//! // Or using .parse()
//! let api_version: ApiVersion = "extensions/v1beta1".parse().unwrap();
//! ```
//!
//! ### Constructing
//!
//! Alternatively, they can be constructed programatically using the
//! [`ApiVersion::new()`] and [`ApiVersion::try_new()`] functions.
//!
//! ```
//! # use std::str::FromStr;
//! use k8s_version::{ApiVersion, Version, Level, Group};
//!
//! let version = Version::new(1, Some(Level::Beta(1)));
//! let group = Group::from_str("extension").unwrap();
//! let api_version = ApiVersion::new(Some(group), version);
//!
//! assert_eq!(api_version.to_string(), "extension/v1beta1");
//!
//! // Or using ::try_new()
//! let version = Version::new(1, Some(Level::Beta(1)));
//! let api_version = ApiVersion::try_new(
//!     Some("extension"),
//!     version
//! ).unwrap();
//!
//! assert_eq!(api_version.to_string(), "extension/v1beta1");
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
