//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in sensible defaults.
//!
pub mod configmap;
pub mod event;
pub mod meta;
pub mod pod;
pub mod resources;

#[deprecated(since = "0.15.0", note = "Please use `builder::configmap::*` instead.")]
pub use configmap::*;

#[deprecated(since = "0.15.0", note = "Please use `builder::event::*` instead.")]
pub use event::*;

#[deprecated(since = "0.15.0", note = "Please use `builder::meta::*` instead.")]
pub use meta::*;

#[deprecated(
    since = "0.15.0",
    note = "Please use `builder::pod::container::*` instead."
)]
pub use pod::container::*;

#[deprecated(
    since = "0.15.0",
    note = "Please use `builder::pod::security::*` instead."
)]
pub use pod::security::*;

#[deprecated(
    since = "0.15.0",
    note = "Please use `builder::pod::volume::*` instead."
)]
pub use pod::volume::*;

#[deprecated(since = "0.15.0", note = "Please use `builder::pod::*` instead.")]
pub use pod::*;
