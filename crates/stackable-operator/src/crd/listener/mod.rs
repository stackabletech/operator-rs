//! This modules provides resource types used to interact with [listener-operator][listener-docs].
//!
//! [listener-docs]: https://docs.stackable.tech/listener-operator/stable/index.html
//! [lvb]: ListenerOperatorVolumeSourceBuilder

#[cfg(doc)]
use k8s_openapi::api::core::v1::{Node, PersistentVolume, PersistentVolumeClaim, Pod, Volume};

#[cfg(doc)]
use crate::builder::pod::volume::ListenerOperatorVolumeSourceBuilder;

mod class;
mod core;
mod listeners;

pub use class::{ListenerClass, ListenerClassVersion};
pub use listeners::{Listener, ListenerVersion, PodListeners, PodListenersVersion};

// Group all v1alpha1 items in one module.
pub mod v1alpha1 {
    pub use super::{class::v1alpha1::*, core::v1alpha1::*, listeners::v1alpha1::*};
}
