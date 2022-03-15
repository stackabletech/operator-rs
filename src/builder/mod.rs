//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in sensible defaults.
//!
pub mod configmap;
pub mod event;
pub mod meta;
pub mod pod;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::configmap::*` in the future"
)]
pub use configmap::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::event::*` in the future"
)]
pub use event::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::meta::*` in the future"
)]
pub use meta::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::pod::container::*` in the future"
)]
pub use pod::container::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::pod::security::*` in the future"
)]
pub use pod::security::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::pod::volume::*` in the future"
)]
pub use pod::volume::*;

#[deprecated(
    since = "0.15.0",
    note = "This is for compatibility. Please use `builder::pod::*` in the future"
)]
pub use pod::*;
