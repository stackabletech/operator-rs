//! This module updates the `UpOrDowngrading` status condition by comparing the current and the
//! target cluster versions.
//!
//! The [`crate::status::Conditions`] and [`crate::status::Versioned`] must be implemented
//! for the custom resource status to ensure generic access.
//!
//! The status field names ("conditions" and "version") are fixed (for patching updates) and should
//! be defined in the operators as follows:
//! ```
//! use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! use stackable_operator::versioning::ProductVersion;
//!
//! #[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
//! pub enum SomeVersion { SomeVersion }
//! #[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
//! pub struct SomeClusterStatus {
//!     pub conditions: Vec<Condition>,
//!     pub version: Option<ProductVersion<SomeVersion>>,
//! }
//! ```
//!
//! Additionally, the product version must implement [`Versioning`] to
//! indicate if upgrade or downgrades are valid, not supported or invalid.
//!
//! This module only provides the tracking of the `ProductVersion` and `Conditions`. Pods etc. are
//! deleted via `delete_illegal_pods` and `delete_excess_pods` in the reconcile crate.
//!
use crate::client::Client;
use crate::conditions::{build_condition, ConditionStatus};
use crate::error::{Error, OperatorResult};
use crate::status::{Conditions, Status, Versioned};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::{Debug, Display};
use strum_macros::AsRefStr;
use tracing::{debug, info, warn};

/// Versioning condition type. Can only contain alphanumeric characters and '-'.
const CONDITION_TYPE: &str = "UpOrDowngrading";

/// This is required to be implemented by the product version of the operators.
pub trait Versioning {
    /// Returns a `VersioningState` that indicates if an upgrade or downgrade is valid, not
    /// supported or invalid.
    fn versioning_state(&self, other: &Self) -> VersioningState;
}

/// Possible return values of the `versioning_state` trait method.
#[derive(Clone, Debug, Eq, PartialEq)]
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

impl<T> ProductVersion<T> {
    /// Builds a `ProductVersion` to be written into the custom resource status.
    ///
    /// # Arguments
    ///
    /// * `current_version` - The optional current version of the cluster.
    /// * `target_version` - The optional target version for upgrading / downgrading the cluster.
    ///
    pub fn new(current: Option<T>, target: Option<T>) -> Self {
        ProductVersion { current, target }
    }
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
        )? {
            (Some(version), Some(condition)) => {
                let updated_resource = client
                    .merge_patch_status(resource, &json!({ "version": version }))
                    .await?;
                return client.set_condition(&updated_resource, condition).await;
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
                        "No upgrade or downgrade required [{}] is still the current_version",
                        target_version
                    ),
                    VersioningConditionReason::Empty.as_ref(),
                    ConditionStatus::False,
                );

                let updated_resource = client.set_condition(resource, condition).await?;

                let v = ProductVersion {
                    current: Some(target_version.clone()),
                    target: None,
                };

                return client
                    .merge_patch_status(&updated_resource, &json!({ "version": v }))
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
    product_version: Option<&ProductVersion<V>>,
    spec_version: V,
    conditions: &[Condition],
) -> OperatorResult<(Option<ProductVersion<V>>, Option<Condition>)>
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
                VersioningConditionReason::InitialInstallation.as_ref(),
                ConditionStatus::True,
            );

            let version = ProductVersion::new(None, Some(spec_version));

            Ok((Some(version), Some(condition)))
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
                VersioningConditionReason::Installing.as_ref(),
                ConditionStatus::True,
            );

            Ok((None, Some(condition)))
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
                        VersioningConditionReason::Upgrading.as_ref(),
                        ConditionStatus::True,
                    );

                    let version = ProductVersion::new(None, Some(spec_version));

                    Ok((Some(version), Some(condition)))
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
                        VersioningConditionReason::Downgrading.as_ref(),
                        ConditionStatus::True,
                    );

                    let version = ProductVersion::new(None, Some(spec_version));

                    Ok((Some(version), Some(condition)))
                }
                VersioningState::NoOp => {
                    let message = format!(
                        "No upgrade required [{}] is still the current_version",
                        current_version
                    );

                    debug!("{}", message);

                    Ok((None, None))
                }
                VersioningState::NotSupported => Err(Error::VersioningError {
                    message: format!(
                        "Up-/Downgrade from [{}] to [{}] not supported but requested in spec. \
                        Please choose a valid version for Up-/Downgrading.",
                        current_version, spec_version
                    ),
                }),
                VersioningState::Invalid(err) => Err(Error::VersioningError {
                    message: format!("Error occurred for versioning: {}", err),
                }),
            }
        }
        _ => Ok((None, None)),
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

#[derive(AsRefStr, Debug)]
enum VersioningConditionReason {
    InitialInstallation,
    Installing,
    Upgrading,
    Downgrading,
    #[strum(serialize = "")]
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResource;
    use rstest::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    const TEST_CLUSTER_YAML: &str = "
        apiVersion: test.stackable.tech/v1alpha1
        kind: TestCluster
        metadata:
          name: simple
        spec:
          test: test
    ";

    #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[kube(
        group = "test.stackable.tech",
        version = "v1alpha1",
        kind = "TestCluster",
        plural = "testclusters",
        namespaced
    )]
    #[kube(status = "TestClusterStatus")]
    pub struct TestClusterSpec {
        pub test: String,
    }

    impl Status<TestClusterStatus> for TestCluster {
        fn status(&self) -> Option<&TestClusterStatus> {
            self.status.as_ref()
        }
        fn status_mut(&mut self) -> &mut Option<TestClusterStatus> {
            &mut self.status
        }
    }

    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
    pub struct TestClusterStatus {
        pub conditions: Vec<Condition>,
        pub version: Option<ProductVersion<TestVersion>>,
    }

    impl Versioned<TestVersion> for TestClusterStatus {
        fn version(&self) -> Option<&ProductVersion<TestVersion>> {
            self.version.as_ref()
        }
        fn version_mut(&mut self) -> &mut Option<ProductVersion<TestVersion>> {
            &mut self.version
        }
    }

    impl Conditions for TestClusterStatus {
        fn conditions(&self) -> &[Condition] {
            self.conditions.as_slice()
        }
        fn conditions_mut(&mut self) -> &mut Vec<Condition> {
            &mut self.conditions
        }
    }

    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        JsonSchema,
        PartialEq,
        Serialize,
        strum_macros::Display,
        strum_macros::EnumString,
    )]
    pub enum TestVersion {
        #[strum(serialize = "1.2.3")]
        V1_2_3,
        #[strum(serialize = "3.2.1")]
        V3_2_1,
        #[strum(serialize = "NotSupported")]
        NotSupported,
        #[strum(serialize = "Invalid")]
        Invalid,
    }

    impl Versioning for TestVersion {
        fn versioning_state(&self, other: &Self) -> VersioningState {
            if *self == TestVersion::V1_2_3 && *other == TestVersion::V3_2_1 {
                VersioningState::ValidUpgrade
            } else if *self == TestVersion::V3_2_1 && *other == TestVersion::V1_2_3 {
                VersioningState::ValidDowngrade
            } else if *self == *other
                && (*self != TestVersion::NotSupported || *self != TestVersion::Invalid)
            {
                VersioningState::NoOp
            } else if *self == TestVersion::NotSupported || *other == TestVersion::NotSupported {
                VersioningState::NotSupported
            } else {
                VersioningState::Invalid("Invalid".to_string())
            }
        }
    }

    #[rstest]
    #[case::initial_installation(
        Some(ProductVersion{ current: None, target: None }),
        TestVersion::V1_2_3,
        Some(ProductVersion { current: None, target: Some(TestVersion::V1_2_3) }),
        (Some(VersioningConditionReason::InitialInstallation.as_ref().to_string()), Some(ConditionStatus::True) )
    )]
    #[case::installation(
        Some(ProductVersion{ current: None, target: Some(TestVersion::V1_2_3) }),
        TestVersion::V1_2_3,
        None,
        (Some(VersioningConditionReason::Installing.as_ref().to_string()), Some(ConditionStatus::True) )
    )]
    #[case::no_op(
        Some(ProductVersion{ current: Some(TestVersion::V1_2_3), target: None }),
        TestVersion::V1_2_3,
        None,
        (None, None)
    )]
    #[case::upgrading(
        Some(ProductVersion{ current: Some(TestVersion::V1_2_3), target: None }),
        TestVersion::V3_2_1,
        Some(ProductVersion { current: None, target: Some(TestVersion::V3_2_1) }),
        (Some(VersioningConditionReason::Upgrading.as_ref().to_string()), Some(ConditionStatus::True) )
    )]
    #[case::downgrading(
        Some(ProductVersion{ current: Some(TestVersion::V3_2_1), target: None }),
        TestVersion::V1_2_3,
        Some(ProductVersion { current: None, target: Some(TestVersion::V1_2_3) }),
        (Some(VersioningConditionReason::Downgrading.as_ref().to_string()), Some(ConditionStatus::True) )
    )]
    fn test_build_version_and_conditions(
        #[case] product_version: Option<ProductVersion<TestVersion>>,
        #[case] spec_version: TestVersion,
        #[case] expected_version: Option<ProductVersion<TestVersion>>,
        // (reason, status)
        #[case] expected_conditions: (Option<String>, Option<ConditionStatus>),
    ) {
        let cluster: TestCluster =
            serde_yaml::from_str(TEST_CLUSTER_YAML).expect("Invalid test cluster definition!");

        let (version, condition) = build_version_and_condition(
            &cluster,
            product_version.as_ref(),
            spec_version,
            vec![].as_slice(),
        )
        .unwrap();

        let (reason, status) = expected_conditions;

        assert_eq!(version, expected_version);
        assert_eq!(reason, condition.as_ref().map(|c| c.reason.clone()));
        assert_eq!(
            status.map(|s| s.to_string()),
            condition.as_ref().map(|c| c.status.clone())
        );
    }

    #[rstest]
    #[case::not_supported(
        Some(ProductVersion{ current: Some(TestVersion::V3_2_1), target: None }),
        TestVersion::NotSupported,
    )]
    #[case::invalid(
        Some(ProductVersion{ current: Some(TestVersion::V3_2_1), target: None }),
        TestVersion::Invalid,
    )]
    fn test_build_version_and_conditions_failing(
        #[case] product_version: Option<ProductVersion<TestVersion>>,
        #[case] spec_version: TestVersion,
    ) {
        let cluster: TestCluster =
            serde_yaml::from_str(TEST_CLUSTER_YAML).expect("Invalid test cluster definition!");

        build_version_and_condition(
            &cluster,
            product_version.as_ref(),
            spec_version,
            vec![].as_slice(),
        )
        .unwrap_err();
    }
}
