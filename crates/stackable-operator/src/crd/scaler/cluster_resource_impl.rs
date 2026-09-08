use k8s_openapi::DeepMerge;

use super::{ScalerState, ScalerStatus, v1alpha1::Scaler};

impl DeepMerge for Scaler {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.metadata, other.metadata);
        // `u16` does not implement `DeepMerge`, so we replace directly.
        self.spec.replicas = other.spec.replicas;
        DeepMerge::merge_from(&mut self.status, other.status);
    }
}

impl DeepMerge for ScalerStatus {
    fn merge_from(&mut self, other: Self) {
        // `u16` does not implement `DeepMerge`, so we replace directly.
        self.replicas = other.replicas;
        DeepMerge::merge_from(&mut self.selector, other.selector);
        DeepMerge::merge_from(&mut self.state, other.state);
        // `Time` does not implement `DeepMerge`, so we replace directly.
        self.last_transition_time = other.last_transition_time;
    }
}

impl DeepMerge for ScalerState {
    fn merge_from(&mut self, other: Self) {
        *self = other;
    }
}
