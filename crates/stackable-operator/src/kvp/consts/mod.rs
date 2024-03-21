//! This module contains various label and annotation related constants used by
//! Kubernetes. Most constants define well-known `app.kubernetes.io/<NAME>`
//! keys. These constants can be used to construct various labels or annotations
//! without sprinkling magic values all over the code.
mod keys;
mod values;

pub use keys::*;
pub use values::*;
