pub mod builder;
pub mod cli;
pub mod client;
#[deprecated(
    since = "0.5.0",
    note = "Start/Stop has been moved to the cluster definition, other commands will be re-added in a different way when needed."
)]
#[allow(deprecated)]
pub mod command;
#[deprecated(
    since = "0.5.0",
    note = "Start/Stop has been moved to the cluster definition, other commands will be re-added in a different way when needed."
)]
#[allow(deprecated)]
pub mod command_controller;

#[deprecated(since = "0.5.0")]
#[allow(deprecated)]
pub mod conditions;

#[deprecated(since = "0.5.0")]
#[allow(deprecated)]
pub mod configmap;
pub mod controller;

#[deprecated(since = "0.5.0")]
#[allow(deprecated)]
pub mod controller_ref;

#[deprecated(since = "0.5.0")]
#[allow(deprecated)]
pub mod controller_utils;

pub mod crd;
pub mod error;

#[deprecated(
    since = "0.5.0",
    note = "Finalizer handling has been introduced in kube.rs 0.58"
)]
#[allow(deprecated)]
pub mod finalizer;

#[deprecated(since = "0.5.0", note = "Only needed for the sticky scheduler")]
#[allow(deprecated)]
pub mod identity;

#[deprecated(since = "0.5.0")]
pub mod k8s_errors;

#[deprecated(since = "0.5.0", note = "Unneeded due move to statefulsets")]
#[allow(deprecated)]
pub mod k8s_utils;
pub mod label_selector;
pub mod labels;
pub mod logging;

#[deprecated(
    since = "0.5.0",
    note = "Functionality to be moved to RoleGroupRef (go talk to Teo if what you need is not yet supported)"
)]
#[allow(deprecated)]
pub mod name_utils;
pub mod namespace;
pub mod pod_utils;
pub mod product_config_utils;

#[deprecated(since = "0.5.0", note = "Unneeded due move to statefulsets")]
#[allow(deprecated)]
pub mod reconcile;
pub mod role_utils;

#[deprecated(since = "0.5.0", note = "Unneeded due move to statefulsets")]
#[allow(deprecated)]
pub mod scheduler;

#[deprecated(since = "0.5.0", note = "Unneeded after changed command handling")]
#[allow(deprecated)]
pub mod status;
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
