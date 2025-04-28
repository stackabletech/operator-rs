use crate::crd::listener::listeners::v1alpha1::ListenerSpec;

impl ListenerSpec {
    pub(super) const fn default_publish_not_ready_addresses() -> Option<bool> {
        Some(true)
    }
}
