//! This module provides builders for various (Kubernetes) objects.
//!
//! They are often not _pure_ builders but contain extra logic to set fields based on others or
//! to fill in defaults that make sense.
//!
pub mod configmap;
pub mod event;
pub mod pod;
pub mod resource;
