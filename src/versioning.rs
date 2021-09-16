//! This module handles up and downgrades for the products handled by the operators.
//!
//! The [`crate::status::Conditions`] and [`crate::status::Versioned`] must be implemented
//! for the custom resource status to ensure generic access.
//!
//! The status field names ("conditions" and "version") are fixed (for patching updates) and should
//! be defined in the operators as follows:
//! ```
//! use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
//! use stackable_operator::versioning::ProductVersion;
//!
//! pub enum SomeVersion { SomeVersion }
//!
//! pub struct SomeClusterStatus {
//!     pub conditions: Vec<Condition>,
//!     pub version: Option<ProductVersion<SomeVersion>>,
//! }
//! ```
//!
//! Additionally, the product version must implement [`crate::versioning:Versioning`] to
//! indicate if upgrade or downgrades are valid, not supported or invalid.
//!
//! This module only provides the tracking of the `ProductVersion` and `Conditions`. Pods etc. are
//! deleted via `delete_illegal_pods` and `delete_excess_pods` in the reconcile crate.
//!
use crate::client::Client;
use crate::conditions::{build_condition, ConditionStatus};
use crate::error::OperatorResult;
use crate::status::{Conditions, Status, Versioned};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::{Debug, Display};
use tracing::{debug, error, info, warn};

/// Versioning condition type. Can only contain alphanumeric characters and '-'.
const CONDITION_TYPE: &str = "UpOrDowngrading";

/// This is required to be implemented by the product version of the operators.
pub trait Versioning {
    /// Returns a `VersioningState` that indicates if an upgrade or downgrade is valid, not
    /// supported or invalid.
    fn versioning_state(&self, other: &Self) -> VersioningState;
}

/// Possible return values of the `versioning_state` trait method.
pub enum VersioningState {
    /// Indicates that the planned upgrade from a lower to higher version is valid and supported.
    ValidUpgrade,
    /// Indicates that the planned downgrade from a higher to lower version is valid and supported.
    ValidDowngrade,
    /// Indicates that no action is required (because the current and target version are equal).
    NoOp,
    /// Indicates that the planned up or downgrade is not supported (e.g. because of version
    /// incompatibility).
    NotSupported,
    /// Indicates that something (e.g. parsing of the current or target version) failed.
    Invalid(String),
}

/// The version of the product provided by the operator. Split into current and target version in
/// order track upgrading and downgrading progress.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductVersion<T> {
    current: Option<T>,
    target: Option<T>,
}

/// Checks the custom resource status (or creates the default status) and processes the contents
/// of the `ProductVersion`. Will update the `ProductVersion` and `Conditions` of the status to
/// signal the upgrading / downgrading progress.
///
/// Returns the updated custom resource for further usage.
///
/// # Arguments
///
/// * `client` - The Kubernetes client.
/// * `resource` - The cluster custom resource.
/// * `spec_version` - The version currently specified in the custom resource.
///
pub async fn init_versioning<T, S, V>(
    client: &Client,
    resource: &T,
    spec_version: V,
) -> OperatorResult<T>
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Status<S>,
    S: Conditions + Debug + Default + Serialize + Versioned<V>,
    V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
{
    if let Some(status) = resource.status() {
        match build_version_and_condition(
            resource,
            status.version(),
            spec_version,
            status.conditions(),
        ) {
            (Some(version), Some(condition)) => {
                client
                    .merge_patch_status(resource, &json!({ "version": version }))
                    .await?;
                return client.set_condition(resource, condition).await;
            }
            (Some(version), None) => {
                return client
                    .merge_patch_status(resource, &json!({ "version": version }))
                    .await;
            }
            (None, Some(condition)) => {
                return client.set_condition(resource, condition).await;
            }
            _ => {}
        }
    }

    Ok(resource.clone())
}

/// Finalizes the `init_versioning`. This is required after e.g. all pods and config maps were
/// created. It will remove the `target_version` from the status `ProductVersion` and set the
/// condition status to false.
///
/// Returns the updated custom resource for further usage.
///
/// # Arguments
///
/// * `client` - The Kubernetes client.
/// * `resource` - The cluster custom resource.
///
pub async fn finalize_versioning<T, S, V>(client: &Client, resource: &T) -> OperatorResult<T>
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()> + Status<S>,
    S: Conditions + Debug + Default + Serialize + Versioned<V>,
    V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
{
    if let Some(status) = resource.status() {
        if let Some(version) = status.version() {
            if let Some(target_version) = &version.target {
                let condition = build_versioning_condition(
                    resource,
                    status.conditions(),
                    &format!(
                        "No upgrade required [{}] is still the current_version",
                        target_version
                    ),
                    "",
                    ConditionStatus::False,
                );

                client.set_condition(resource, condition).await?;

                let v = ProductVersion {
                    current: Some(target_version.clone()),
                    target: None,
                };

                return client
                    .merge_patch_status(resource, &json!({ "version": v }))
                    .await;
            }
        }
    }

    Ok(resource.clone())
}

/// Checks that the custom resource status (or creates the default status) and processes the contents
/// of the `ProductVersion`. Will update the `ProductVersion` and `Conditions` of the status to
/// signal the upgrading / downgrading progress.
///
/// # Arguments
///
/// * `resource` - The cluster custom resource.
/// * `product_version` - The `ProductVersion` set in the status field `version`.
/// * `spec_version` - The version currently specified in the custom resource.
/// * `conditions` - The conditions from the custom resource status.
///
fn build_version_and_condition<T, V>(
    resource: &T,
    product_version: &Option<ProductVersion<V>>,
    spec_version: V,
    conditions: &[Condition],
) -> (Option<ProductVersion<V>>, Option<Condition>)
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
    V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
{
    return match (
        product_version.as_ref().and_then(|v| v.current.as_ref()),
        product_version.as_ref().and_then(|v| v.target.as_ref()),
    ) {
        (None, None) => {
            // No current_version and no target_version -> must be initial installation.
            // We set the Upgrading condition and the target_version to the version from spec.
            let message = format!("Initial installation of version [{}]", spec_version);

            info!("{}", message);

            let condition = build_versioning_condition(
                resource,
                conditions,
                &message,
                "InitialInstallation",
                ConditionStatus::True,
            );

            let version: ProductVersion<V> = build_version(None, Some(spec_version));

            (Some(version), Some(condition))
        }
        (None, Some(target_version)) => {
            // No current_version but a target_version means we are still doing the initial
            // installation. Will continue working towards that goal even if another version
            // was set in the meantime.
            let message = format!("Installing version [{}]", spec_version);

            debug!("{}", message);

            if &spec_version != target_version {
                warn!("A new target version ([{}]) was requested while we still do the installation to [{}],\
                       finishing running upgrade first", spec_version, target_version)
            }
            // We do this here to update the observedGeneration if needed
            let condition = build_versioning_condition(
                resource,
                conditions,
                &message,
                "Installing",
                ConditionStatus::True,
            );

            (None, Some(condition))
        }
        (Some(current_version), None) => {
            // We are at a stable version but have no target_version set.
            // This will be the normal state.
            // We'll check if there is a different version in spec and if it is will
            // set it in target_version, but only if it's actually a compatible upgrade.
            let versioning_option = current_version.versioning_state(&spec_version);
            match versioning_option {
                VersioningState::ValidUpgrade => {
                    let message = format!(
                        "Upgrading from [{}] to [{}]",
                        current_version, &spec_version
                    );

                    info!("{}", message);

                    let condition = build_versioning_condition(
                        resource,
                        conditions,
                        &message,
                        "Upgrading",
                        ConditionStatus::True,
                    );

                    let version = build_version(None, Some(spec_version));

                    (Some(version), Some(condition))
                }
                VersioningState::ValidDowngrade => {
                    let message = format!(
                        "Downgrading from [{}] to [{}]",
                        current_version, spec_version
                    );

                    info!("{}", message);

                    let condition = build_versioning_condition(
                        resource,
                        conditions,
                        &message,
                        "Downgrading",
                        ConditionStatus::True,
                    );

                    let version = build_version(None, Some(spec_version));

                    (Some(version), Some(condition))
                }
                VersioningState::NoOp => {
                    let message = format!(
                        "No upgrade required [{}] is still the current_version",
                        current_version
                    );

                    debug!("{}", message);

                    (None, None)
                }
                VersioningState::NotSupported => {
                    warn!("Up-/Downgrade from [{}] to [{}] not possible but requested in spec: Ignoring, will continue \
                           reconcile as if the invalid version weren't set", current_version, spec_version);
                    (None, None)
                }
                VersioningState::Invalid(err) => {
                    // TODO: throw error
                    error!("Error occurred for versioning: {}", err);
                    (None, None)
                }
            }
        }
        _ => (None, None),
    };
}

/// Builds a condition for versioning. It basically forwards every parameter and uses the
/// fixed `CONDITION_TYPE` to identify the versioning condition.
///
/// # Arguments
///
/// * `resource` - The cluster custom resource.
/// * `conditions` - The conditions from the custom resource status.
/// * `message` - The message set in the conditions.
/// * `reason` - The reason set in the conditions
/// * `status` - The status set in the conditions.
///
fn build_versioning_condition<T>(
    resource: &T,
    conditions: &[Condition],
    message: &str,
    reason: &str,
    status: ConditionStatus,
) -> Condition
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
{
    build_condition(
        resource,
        Some(conditions),
        message.to_string(),
        reason.to_string(),
        status,
        CONDITION_TYPE.to_string(),
    )
}

/// Builds a `ProductVersion` to be written into the custom resource status.
///
/// # Arguments
///
/// * `current_version` - The optional current version of the cluster.
/// * `target_version` - The optional target version for upgrading / downgrading the cluster.
///
fn build_version<V>(current_version: Option<V>, target_version: Option<V>) -> ProductVersion<V>
where
    V: Clone,
{
    ProductVersion {
        current: current_version,
        target: target_version,
    }
}
