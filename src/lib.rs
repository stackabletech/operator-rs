pub mod builder;
pub mod cli;
pub mod client;
pub mod commons;
pub mod crd;
pub mod error;
pub mod label_selector;
pub mod labels;
pub mod logging;
pub mod namespace;
pub mod pod_utils;
pub mod product_config_utils;
pub mod resources;
pub mod role_utils;
pub mod utils;
pub mod validation;

pub use crate::crd::CustomResourceExt;

pub use ::k8s_openapi;
pub use ::kube;
pub use ::product_config;
pub use ::schemars;
