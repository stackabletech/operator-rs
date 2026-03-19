//! Builder helpers for constructing `HorizontalPodAutoscaler` objects and initializing
//! [`StackableScaler`] status.
//!
//! Product operators use [`build_hpa_from_user_spec`] to create an HPA whose
//! `scaleTargetRef` always points at the correct [`StackableScaler`], and
//! [`initialize_scaler_status`] to seed the scaler's status subresource so that
//! the first reconcile does not see `replicas: 0` and trigger an unintended scale-to-zero.

use k8s_openapi::api::autoscaling::v2::{
    CrossVersionObjectReference, HorizontalPodAutoscaler, HorizontalPodAutoscalerSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{OwnerReference, Time};
use k8s_openapi::jiff::Timestamp;
use snafu::{ResultExt, Snafu};

use crate::builder::meta::ObjectMetaBuilder;
use crate::client::Client;
use crate::kvp::{Label, Labels, consts::K8S_APP_MANAGED_BY_KEY};

use super::builder::BuildScalerError;
use super::v1alpha1::StackableScaler;
use super::{ScalerStage, ScalerState, StackableScalerStatus};

/// Errors returned by [`initialize_scaler_status`].
#[derive(Debug, Snafu)]
pub enum InitializeStatusError {
    /// The Kubernetes status patch for the [`StackableScaler`] failed.
    #[snafu(display("failed to patch initial StackableScaler status"))]
    PatchStatus {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
    },
}

/// Build a [`CrossVersionObjectReference`] that points at a [`StackableScaler`].
///
/// The returned reference is suitable for use as the `scaleTargetRef` of a
/// `HorizontalPodAutoscaler`.
///
/// # Parameters
///
/// - `scaler_name`: The `metadata.name` of the target [`StackableScaler`].
/// - `group`: The API group (e.g. `"autoscaling.stackable.tech"`).
/// - `version`: The API version (e.g. `"v1alpha1"`).
pub fn scale_target_ref(
    scaler_name: &str,
    group: &str,
    version: &str,
) -> CrossVersionObjectReference {
    CrossVersionObjectReference {
        kind: "StackableScaler".to_string(),
        name: scaler_name.to_string(),
        api_version: Some(format!("{group}/{version}")),
    }
}

/// Build a [`HorizontalPodAutoscaler`] from a user-provided spec, overwriting
/// `scaleTargetRef` so it always points at the correct [`StackableScaler`].
///
/// The generated HPA name follows the convention
/// `{cluster_name}-{role}-{role_group}-hpa`.
///
/// # Labels
///
/// The same five `app.kubernetes.io` labels used by [`build_scaler`](super::build_scaler)
/// are applied:
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
// distinct Kubernetes metadata fields required on an HPA. Grouping them into a struct
// would just push the field list one level deeper without reducing cognitive load,
// since callers already have each value as a separate variable.
#[allow(clippy::too_many_arguments)]
pub fn build_hpa_from_user_spec(
    user_spec: &HorizontalPodAutoscalerSpec,
    target_ref: &CrossVersionObjectReference,
    cluster_name: &str,
    app_name: &str,
    namespace: &str,
    role: &str,
    role_group: &str,
    owner_ref: &OwnerReference,
    managed_by: &str,
) -> Result<HorizontalPodAutoscaler, BuildScalerError> {
    let hpa_name = format!("{cluster_name}-{role}-{role_group}-hpa");

    let mut labels = Labels::common(app_name, cluster_name).context(super::builder::LabelSnafu)?;
    labels.insert(Label::component(role).context(super::builder::LabelSnafu)?);
    labels.insert(Label::role_group(role_group).context(super::builder::LabelSnafu)?);
    labels.insert(
        Label::try_from((K8S_APP_MANAGED_BY_KEY, managed_by))
            .context(super::builder::LabelSnafu)?,
    );

    let metadata = ObjectMetaBuilder::new()
        .name(&hpa_name)
        .namespace(namespace)
        .ownerreference(owner_ref.clone())
        .with_labels(labels)
        .build();

    let mut spec = user_spec.clone();
    spec.scale_target_ref = target_ref.clone();

    Ok(HorizontalPodAutoscaler {
        metadata,
        spec: Some(spec),
        status: None,
    })
}

/// Patch a freshly created [`StackableScaler`]'s status to prevent scale-to-zero.
///
/// When a scaler is first created it has no status. Without this initialization,
/// reading `status.replicas` would yield `0`, causing the StatefulSet to scale down
/// to zero pods. This function seeds the status with the current replica count and
/// an `Idle` stage so that the first reconcile sees the correct baseline.
///
/// # Parameters
///
/// - `client`: Kubernetes client for the status patch operation.
/// - `scaler`: The freshly created [`StackableScaler`] resource.
/// - `current_replicas`: The current replica count of the managed StatefulSet.
/// - `selector`: Pod label selector string for HPA pod counting (e.g.
///   `"app=myproduct,roleGroup=default"`).
///
/// # Errors
///
/// Returns [`InitializeStatusError::PatchStatus`] if the Kubernetes status patch fails.
pub async fn initialize_scaler_status(
    client: &Client,
    scaler: &StackableScaler,
    current_replicas: i32,
    selector: &str,
) -> Result<(), InitializeStatusError> {
    let status = StackableScalerStatus {
        replicas: current_replicas,
        selector: Some(selector.to_string()),
        desired_replicas: Some(current_replicas),
        previous_replicas: None,
        current_state: Some(ScalerState {
            stage: ScalerStage::Idle,
            last_transition_time: Time(Timestamp::now()),
        }),
    };

    client
        .apply_patch_status("stackable-operator", scaler, &status)
        .await
        .map(|_| ())
        .context(PatchStatusSnafu)
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::autoscaling::v2::{
        CrossVersionObjectReference, HorizontalPodAutoscalerSpec,
    };
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

    fn test_target_ref() -> CrossVersionObjectReference {
        scale_target_ref(
            "my-nifi-nodes-default-scaler",
            "autoscaling.stackable.tech",
            "v1alpha1",
        )
    }

    #[test]
    fn scale_target_ref_points_to_scaler() {
        let target = scale_target_ref(
            "my-nifi-nodes-default-scaler",
            "autoscaling.stackable.tech",
            "v1alpha1",
        );

        assert_eq!(target.kind, "StackableScaler");
        assert_eq!(target.name, "my-nifi-nodes-default-scaler");
        assert_eq!(
            target.api_version.as_deref(),
            Some("autoscaling.stackable.tech/v1alpha1")
        );
    }

    #[test]
    fn build_hpa_overwrites_scale_target_ref() {
        let wrong_ref = CrossVersionObjectReference {
            kind: "Deployment".to_string(),
            name: "wrong-target".to_string(),
            api_version: Some("apps/v1".to_string()),
        };
        let user_spec = HorizontalPodAutoscalerSpec {
            max_replicas: 10,
            scale_target_ref: wrong_ref,
            ..Default::default()
        };
        let correct_ref = test_target_ref();

        let hpa = build_hpa_from_user_spec(
            &user_spec,
            &correct_ref,
            "my-nifi",
            "nifi",
            "default",
            "nodes",
            "default",
            &test_owner_ref(),
            "nifi-operator",
        )
        .expect("build_hpa_from_user_spec should succeed");

        let spec = hpa.spec.expect("spec should be set");
        assert_eq!(spec.scale_target_ref.kind, "StackableScaler");
        assert_eq!(spec.scale_target_ref.name, "my-nifi-nodes-default-scaler");
        assert_eq!(
            spec.scale_target_ref.api_version.as_deref(),
            Some("autoscaling.stackable.tech/v1alpha1")
        );
        // Original max_replicas should be preserved.
        assert_eq!(spec.max_replicas, 10);
    }

    #[test]
    fn build_hpa_sets_required_labels() {
        let user_spec = HorizontalPodAutoscalerSpec {
            max_replicas: 5,
            scale_target_ref: CrossVersionObjectReference {
                kind: "Deployment".to_string(),
                name: "placeholder".to_string(),
                api_version: None,
            },
            ..Default::default()
        };
        let target_ref = test_target_ref();

        let hpa = build_hpa_from_user_spec(
            &user_spec,
            &target_ref,
            "my-nifi",
            "nifi",
            "production",
            "nodes",
            "workers",
            &test_owner_ref(),
            "nifi-operator",
        )
        .expect("build_hpa_from_user_spec should succeed");

        let labels = hpa.metadata.labels.as_ref().expect("labels should be set");

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
            Some(&"workers".to_string()),
            "app.kubernetes.io/role-group should be the role_group"
        );
    }

    #[test]
    fn build_hpa_generates_correct_name() {
        let user_spec = HorizontalPodAutoscalerSpec {
            max_replicas: 5,
            scale_target_ref: CrossVersionObjectReference {
                kind: "Deployment".to_string(),
                name: "placeholder".to_string(),
                api_version: None,
            },
            ..Default::default()
        };
        let target_ref = test_target_ref();

        let hpa = build_hpa_from_user_spec(
            &user_spec,
            &target_ref,
            "my-nifi",
            "nifi",
            "production",
            "nodes",
            "workers",
            &test_owner_ref(),
            "nifi-operator",
        )
        .expect("build_hpa_from_user_spec should succeed");

        assert_eq!(
            hpa.metadata.name.as_deref(),
            Some("my-nifi-nodes-workers-hpa")
        );
        assert_eq!(hpa.metadata.namespace.as_deref(), Some("production"));
    }

    // `initialize_scaler_status` requires a running Kubernetes cluster and is not
    // unit-testable. It is tested indirectly via the scaler reconciler integration
    // tests and the NiFi operator end-to-end tests.
}
