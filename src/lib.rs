pub mod builder;
pub mod cli;
pub mod client;
pub mod crd;
pub mod error;
pub mod label_selector;
pub mod labels;
pub mod logging;
pub mod namespace;
pub mod opa;
pub mod pod_utils;
pub mod product_config_utils;
pub mod role_utils;
pub mod utils;
pub mod validation;

mod resources;
#[deprecated(since = "0.5.0", note = "Unneeded due move to statefulsets")]
#[allow(deprecated)]
pub mod versioning;

pub use crate::crd::CustomResourceExt;

pub use ::k8s_openapi;
pub use ::kube;
pub use ::product_config;
pub use ::schemars;
