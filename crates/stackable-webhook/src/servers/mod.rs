//! Contains high-level ready-to-use webhook server implementations for specific
//! purposes.
mod conversion;

pub use conversion::{ConversionWebhookError, ConversionWebhookServer};
pub use kube::core::conversion::ConversionReview;
