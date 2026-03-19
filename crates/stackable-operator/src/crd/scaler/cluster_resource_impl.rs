use k8s_openapi::DeepMerge;

use super::{ScalerStage, ScalerState, StackableScalerStatus, v1alpha1::StackableScaler};

impl DeepMerge for StackableScaler {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.metadata, other.metadata);
        DeepMerge::merge_from(&mut self.spec.replicas, other.spec.replicas);
        DeepMerge::merge_from(&mut self.status, other.status);
    }
}

impl DeepMerge for StackableScalerStatus {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.replicas, other.replicas);
        DeepMerge::merge_from(&mut self.selector, other.selector);
        DeepMerge::merge_from(&mut self.desired_replicas, other.desired_replicas);
        DeepMerge::merge_from(&mut self.previous_replicas, other.previous_replicas);
        DeepMerge::merge_from(&mut self.current_state, other.current_state);
    }
}

impl DeepMerge for ScalerState {
    fn merge_from(&mut self, other: Self) {
        DeepMerge::merge_from(&mut self.stage, other.stage);
        // `Time` does not implement `DeepMerge`, so we replace directly.
        self.last_transition_time = other.last_transition_time;
    }
}

impl DeepMerge for ScalerStage {
    fn merge_from(&mut self, other: Self) {
        *self = other;
    }
}
