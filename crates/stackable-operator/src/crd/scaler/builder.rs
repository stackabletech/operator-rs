//! Builder helper for constructing [`StackableScaler`] objects with proper metadata.
//!
//! Product operators use [`build_scaler`] to create a `StackableScaler` for each
//! auto-scaled role group, ensuring that all required labels are set so that
//! [`ClusterResources::add`](crate::cluster_resources::ClusterResources) validation passes.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use snafu::{ResultExt, Snafu};

use crate::{
    builder::meta::ObjectMetaBuilder,
    kvp::{Label, LabelError, Labels, consts::K8S_APP_MANAGED_BY_KEY},
};

use super::v1alpha1::{StackableScaler, StackableScalerSpec};

/// Error returned by [`build_scaler`].
#[derive(Debug, Snafu)]
pub enum BuildScalerError {
    /// A label value failed validation.
    #[snafu(display("failed to construct label for scaler"))]
    Label { source: LabelError },

    /// The metadata builder failed (e.g. missing owner reference fields).
    #[snafu(display("failed to build ObjectMeta for scaler"))]
    ObjectMeta { source: crate::builder::meta::Error },
}

/// Constructs a [`StackableScaler`] with the required labels and owner reference.
///
/// The generated scaler name follows the convention
/// `{cluster_name}-{role}-{role_group}-scaler`.
///
/// # Labels
///
/// The following `app.kubernetes.io` labels are set:
///
/// | Key | Value |
/// |-----|-------|
/// | `app.kubernetes.io/name` | `app_name` |
/// | `app.kubernetes.io/instance` | `cluster_name` |
/// | `app.kubernetes.io/managed-by` | `managed_by` |
/// | `app.kubernetes.io/component` | `role` |
/// | `app.kubernetes.io/role-group` | `role_group` |
///
/// # Errors
///
/// Returns [`BuildScalerError::Label`] if any label value is invalid.
/// Returns [`BuildScalerError::ObjectMeta`] if the owner reference cannot be set.
// `clippy::too_many_arguments` suppressed: these parameters correspond 1:1 to the
// distinct Kubernetes metadata fields required on a StackableScaler. Grouping them
// into a struct would just push the field list one level deeper without reducing
// cognitive load, since callers already have each value as a separate variable.
#[allow(clippy::too_many_arguments)]
pub fn build_scaler(
    cluster_name: &str,
    app_name: &str,
    namespace: &str,
    role: &str,
    role_group: &str,
    initial_replicas: i32,
    owner_ref: &OwnerReference,
    managed_by: &str,
) -> Result<StackableScaler, BuildScalerError> {
    let scaler_name = format!("{cluster_name}-{role}-{role_group}-scaler");

    // Build the label set: name + instance + component + role-group + managed-by
    let mut labels = Labels::common(app_name, cluster_name).context(LabelSnafu)?;
    labels.insert(Label::component(role).context(LabelSnafu)?);
    labels.insert(Label::role_group(role_group).context(LabelSnafu)?);
    labels.insert(Label::try_from((K8S_APP_MANAGED_BY_KEY, managed_by)).context(LabelSnafu)?);

    let metadata = ObjectMetaBuilder::new()
        .name(&scaler_name)
        .namespace(namespace)
        .ownerreference(owner_ref.clone())
        .with_labels(labels)
        .build();

    Ok(StackableScaler {
        metadata,
        spec: StackableScalerSpec {
            replicas: initial_replicas,
        },
        status: None,
    })
}

#[cfg(test)]
mod tests {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;

    use super::*;

    fn test_owner_ref() -> OwnerReference {
        OwnerReference {
            api_version: "nifi.stackable.tech/v1alpha1".to_string(),
            kind: "NifiCluster".to_string(),
            name: "my-nifi".to_string(),
            uid: "abc-123".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }
    }

    #[test]
    fn build_scaler_sets_replicas() {
        let owner_ref = test_owner_ref();
        let scaler = build_scaler(
            "my-nifi",
            "nifi",
            "default",
            "nodes",
            "default",
            3,
            &owner_ref,
            "nifi-operator",
        )
        .expect("build_scaler should succeed");

        assert_eq!(scaler.spec.replicas, 3);
    }

    #[test]
    fn build_scaler_sets_owner_reference() {
        let owner_ref = test_owner_ref();
        let scaler = build_scaler(
            "my-nifi",
            "nifi",
            "default",
            "nodes",
            "default",
            1,
            &owner_ref,
            "nifi-operator",
        )
        .expect("build_scaler should succeed");

        let refs = scaler
            .metadata
            .owner_references
            .as_ref()
            .expect("owner_references should be set");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "my-nifi");
        assert_eq!(refs[0].kind, "NifiCluster");
        assert_eq!(refs[0].uid, "abc-123");
    }

    #[test]
    fn build_scaler_sets_required_labels() {
        let owner_ref = test_owner_ref();
        let scaler = build_scaler(
            "my-nifi",
            "nifi",
            "default",
            "nodes",
            "default",
            1,
            &owner_ref,
            "nifi-operator",
        )
        .expect("build_scaler should succeed");

        let labels = scaler
            .metadata
            .labels
            .as_ref()
            .expect("labels should be set");

        assert_eq!(
            labels.get("app.kubernetes.io/name"),
            Some(&"nifi".to_string()),
            "app.kubernetes.io/name should be the app_name"
        );
        assert_eq!(
            labels.get("app.kubernetes.io/instance"),
            Some(&"my-nifi".to_string()),
            "app.kubernetes.io/instance should be the cluster_name"
        );
        assert_eq!(
            labels.get("app.kubernetes.io/managed-by"),
            Some(&"nifi-operator".to_string()),
            "app.kubernetes.io/managed-by should be managed_by"
        );
        assert_eq!(
            labels.get("app.kubernetes.io/component"),
            Some(&"nodes".to_string()),
            "app.kubernetes.io/component should be the role"
        );
        assert_eq!(
            labels.get("app.kubernetes.io/role-group"),
            Some(&"default".to_string()),
            "app.kubernetes.io/role-group should be the role_group"
        );
    }

    #[test]
    fn build_scaler_generates_correct_name() {
        let owner_ref = test_owner_ref();
        let scaler = build_scaler(
            "my-nifi",
            "nifi",
            "production",
            "nodes",
            "workers",
            5,
            &owner_ref,
            "nifi-operator",
        )
        .expect("build_scaler should succeed");

        assert_eq!(
            scaler.metadata.name.as_deref(),
            Some("my-nifi-nodes-workers-scaler")
        );
        assert_eq!(scaler.metadata.namespace.as_deref(), Some("production"));
    }

    #[test]
    fn build_scaler_status_is_none() {
        let owner_ref = test_owner_ref();
        let scaler = build_scaler(
            "my-nifi",
            "nifi",
            "default",
            "nodes",
            "default",
            1,
            &owner_ref,
            "nifi-operator",
        )
        .expect("build_scaler should succeed");

        assert!(
            scaler.status.is_none(),
            "status should be None on a newly built scaler"
        );
    }
}
