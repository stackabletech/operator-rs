use crate::kube::Resource;
use const_format::concatcp;
use std::collections::HashSet;
use std::sync::Arc;
const APP_STACKABLE_LABEL_BASE: &str = "stackable.tech/";

/// The name of the annotation that controls whether or not a cluster may be reconciled at this time
const PAUSE_RECONCILIATION_ANNOTATION: &str =
    concatcp!(APP_STACKABLE_LABEL_BASE, "pause-reconciliation");

/// The name of the annotation that controls whether or not a cluster is stopped
const STOP_CLUSTER_ANNOTATION: &str = concatcp!(APP_STACKABLE_LABEL_BASE, "stop");

#[derive(Eq, PartialEq, Hash)]
pub enum ClusterState {
    ReconciliationPaused {},
    Stopped {},
}

pub fn get_cluster_state_flags<T>(obj: &Arc<T>) -> HashSet<ClusterState>
where
    T: Resource,
{
    let mut cluster_flags = HashSet::new();

    // Check if there are annotations at all, if not, no flags can be set
    if let Some(annotations) = &obj.meta().annotations {
        // Check if reconciliation paused flag is set
        if let Some(pause_annotation) = annotations.get(PAUSE_RECONCILIATION_ANNOTATION) {
            tracing::info!("found stop annotation on object, checking value of set to \"true\"");
            if pause_annotation.to_lowercase() == "true" {
                cluster_flags.insert(ClusterState::ReconciliationPaused {});
            }
        };
        // Check if cluster stop flag is set
        if let Some(stop_annotation) = annotations.get(STOP_CLUSTER_ANNOTATION) {
            tracing::info!("found stop annotation on object, checking value of set to \"true\"");
            if stop_annotation.to_lowercase() == "true" {
                tracing::debug!("value == \"true\"");
                cluster_flags.insert(ClusterState::Stopped {});
            }
        };
    }
    cluster_flags
}
