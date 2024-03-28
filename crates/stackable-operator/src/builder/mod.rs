//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in sensible defaults.
//!
pub mod configmap;
pub mod event;
pub mod meta;
pub mod pdb;
pub mod pod;
