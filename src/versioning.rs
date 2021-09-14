//! This module handles up and downgrades for the operator products.
use crate::client::Client;
use crate::conditions::{build_condition, ConditionStatus};
use crate::error::OperatorResult;
use crate::status::{Conditions, Versioned};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::{Debug, Display};
use tracing::{debug, error, info, trace, warn};

pub trait Versioning {
    fn versioning_state(&self, other: &Self) -> VersioningState;
}

pub enum VersioningState {
    ValidUpgrade,
    ValidDowngrade,
    NoOp,
    NotSupported,
    Invalid(String),
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version<T> {
    current_version: Option<T>,
    target_version: Option<T>,
}

pub struct StatusVersionManager<'a, T>
where
    T: Resource,
{
    client: &'a Client,
    resource: &'a T,
}

impl<'a, T> StatusVersionManager<'a, T>
where
    T: Clone + Debug + DeserializeOwned + Resource<DynamicType = ()>,
{
    pub fn new(client: &'a Client, resource: &'a T) -> Self {
        StatusVersionManager { client, resource }
    }

    pub async fn process<S, V>(
        &self,
        cluster_status: Option<S>,
        spec_version: V,
    ) -> OperatorResult<()>
    where
        S: Conditions + Debug + Default + Serialize + Versioned<V>,
        V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
    {
        // init the status if not available yet
        let status = match cluster_status {
            Some(status) => status,
            None => {
                let default_status = S::default();
                self.client
                    .merge_patch_status(self.resource, &default_status)
                    .await?;
                default_status
            }
        };

        let (version, condition) = self.build_version_and_condition(
            &status
                .version()
                .as_ref()
                .and_then(|v| v.current_version.clone()),
            &status
                .version()
                .as_ref()
                .and_then(|v| v.target_version.clone()),
            spec_version,
            status.conditions(),
        );

        if let Some(version) = version {
            self.client
                .merge_patch_status(self.resource, &json!({ "version": version }))
                .await?;
        }

        if let Some(condition) = condition {
            self.client.set_condition(self.resource, condition).await?;
        }

        Ok(())
    }

    fn build_version_and_condition<V>(
        &self,
        current_version: &Option<V>,
        target_version: &Option<V>,
        spec_version: V,
        conditions: &[Condition],
    ) -> (Option<Version<V>>, Option<Condition>)
    where
        V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
    {
        return match (current_version, target_version) {
            (None, None) => {
                // No current_version and no target_version -> must be initial installation.
                // We set the Upgrading condition and the target_version to the version from spec.
                info!(
                    "Initial installation, now moving towards version [{}]",
                    spec_version
                );

                let condition = self.build_versioning_condition(
                    conditions,
                    &format!("Initial installation to version [{}]", spec_version),
                    "InitialInstallation",
                    ConditionStatus::True,
                );

                let version: Version<V> = build_version(None, Some(spec_version));

                (Some(version), Some(condition))
            }
            (None, Some(target_version)) => {
                // No current_version but a target_version means we are still doing the initial
                // installation. Will continue working towards that goal even if another version
                // was set in the meantime.
                debug!(
                    "Initial installation, still moving towards version [{}]",
                    target_version
                );
                if &spec_version != target_version {
                    info!("A new target version ([{}]) was requested while we still do the initial installation to [{}],\
                          finishing running upgrade first", spec_version, target_version)
                }
                // We do this here to update the observedGeneration if needed
                let condition = self.build_versioning_condition(
                    conditions,
                    &format!("Initial installation to version [{}]", target_version),
                    "InitialInstallation",
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

                        let condition = self.build_versioning_condition(
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

                        let condition = self.build_versioning_condition(
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
                        trace!("{}", message);
                        let condition = self.build_versioning_condition(
                            conditions,
                            &message,
                            "",
                            ConditionStatus::False,
                        );
                        (None, Some(condition))
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

    pub async fn finalize<S, V>(&self, cluster_status: Option<S>) -> OperatorResult<()>
    where
        S: Conditions + Debug + Default + Serialize + Versioned<V>,
        V: Clone + Debug + Display + PartialEq + Serialize + Versioning,
    {
        // If we reach here it means all pods must be running on target_version.
        // We can now set current_version to target_version (if target_version was set) and
        // target_version to None
        if let Some(status) = &cluster_status {
            if let Some(version) = status.version() {
                if let Some(target_version) = &version.target_version {
                    let condition = self.build_versioning_condition(
                        status.conditions(),
                        &format!(
                            "No upgrade required [{}] is still the current_version",
                            target_version
                        ),
                        "",
                        ConditionStatus::False,
                    );

                    self.client.set_condition(self.resource, condition).await?;

                    let v = Version {
                        current_version: Some(target_version.clone()),
                        target_version: None,
                    };

                    self.client
                        .merge_patch_status(self.resource, &json!({ "version": v }))
                        .await?;
                }
            }
        }

        Ok(())
    }

    fn build_versioning_condition(
        &self,
        conditions: &[Condition],
        message: &str,
        reason: &str,
        status: ConditionStatus,
    ) -> Condition {
        build_condition(
            self.resource,
            Some(conditions),
            message.to_string(),
            reason.to_string(),
            status,
            "UpOrDowngrading".to_string(),
        )
    }
}

fn build_version<V>(current_version: Option<V>, target_version: Option<V>) -> Version<V>
where
    V: Clone,
{
    Version {
        current_version,
        target_version,
    }
}
