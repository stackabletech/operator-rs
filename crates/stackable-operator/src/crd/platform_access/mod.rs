//! CRD fragments for resources managed by product agents, and for the credentials the agents use.

pub mod anonymous;
pub mod management;
pub mod tls;

pub mod v1alpha1 {
    pub use super::{anonymous::v1alpha1::*, management::v1alpha1::*, tls::v1alpha1::*};
}
