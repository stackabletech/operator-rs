//! This module offers methods to build and create/update configmaps.
//!
//! ConfigMaps using this module are required to implement certain labels to be identifiable via
//! a LabelSelector:
//! * `labels::APP_NAME_LABEL`
//! * `labels::APP_INSTANCE_LABEL`
//! * `labels::APP_COMPONENT_LABEL`
//! * `labels::APP_ROLE_GROUP_LABEL`
//! * `labels::APP_MANAGED_BY_LABEL`
//! * `configmap::CONFIGMAP_TYPE_LABEL`
//!
//! ConfigMap names should be created via `name_utils::build_resource_name`. The name represents
//! the metadata.generate_name.
//!
//! To have the 'full' name the ConfigMaps have to be created before mounting them into pods.
//! The generate_name from `name_utils::build_resource_name` cannot be used as mount name.   
//!
use crate::builder::{ConfigMapBuilder, ObjectMetaBuilder};
use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::labels;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::Resource;
use lazy_static::lazy_static;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use tracing::{debug, warn};

/// This is a required label to set in the configmaps to differentiate ConfigMaps for e.g.
/// config, data, ids etc.
pub const CONFIGMAP_TYPE_LABEL: &str = "configmap.stackable.tech/type";
/// This label will be set automatically to track the content of the ConfigMap.
pub const CONFIGMAP_HASH_LABEL: &str = "configmap.stackable.tech/hash";

lazy_static! {
    static ref REQUIRED_LABELS: Vec<&'static str> = {
        vec![
            labels::APP_NAME_LABEL,
            labels::APP_INSTANCE_LABEL,
            labels::APP_COMPONENT_LABEL,
            labels::APP_ROLE_GROUP_LABEL,
            labels::APP_MANAGED_BY_LABEL,
            CONFIGMAP_TYPE_LABEL,
        ]
    };
}

/// This method can be used to build a ConfigMap. In order to create, update or delete a config
/// map, it has to be uniquely identifiable via a LabelSelector.
/// That is why the `labels` must contain at least the following labels:
/// * `labels::APP_NAME_LABEL`
/// * `labels::APP_INSTANCE_LABEL`
/// * `labels::APP_COMPONENT_LABEL`
/// * `labels::APP_ROLE_GROUP_LABEL`
/// * `labels::APP_MANAGED_BY_LABEL`
/// * `configmap::CONFIGMAP_TYPE_LABEL`
///
/// # Arguments
///
/// - `cluster` - The Cluster resource object.
/// - `name` - The ConfigMap name (generate_name).
/// - `namespace` - The ConfigMap namespace.
/// - `labels` - The ConfigMap labels.
/// - `data` - The ConfigMap data.
///
pub fn build_config_map<T>(
    cluster: &T,
    name: &str,
    namespace: &str,
    labels: BTreeMap<String, String>,
    data: BTreeMap<String, String>,
) -> OperatorResult<ConfigMap>
where
    T: Resource<DynamicType = ()>,
{
    check_labels(name, &labels)?;

    ConfigMapBuilder::new()
        .metadata(
            ObjectMetaBuilder::new()
                .generate_name(name)
                .ownerreference_from_resource(cluster, Some(true), Some(true))?
                .namespace(namespace)
                .with_labels(labels)
                .build(),
        )
        .data(data)
        .build()
}

/// This method can be used to ensure that a ConfigMap exists and has the specified content.
///
/// If a ConfigMap with the specified name does not exist it will be created.
///
/// Should a ConfigMap with the specified name already exist the content is retrieved and
/// compared with the content from `config_map`, if content differs the existing ConfigMap is
/// updated.
///
/// The ConfigMap labels and data are hashed and added as configmap::CONFIGMAP_HASH_LABEL in
/// order to keep track of changes.
///
/// Returns `Ok(ConfigMap)` if created or updated. Otherwise error.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The ConfigMap to create or update.
///
pub async fn create_config_map(
    client: &Client,
    mut config_map: ConfigMap,
) -> OperatorResult<ConfigMap> {
    let hash = hash_config_map(&config_map);

    return if let Some(mut existing_config_map) = find_config_map(client, &config_map).await? {
        debug!(
            "Found an existing configmap [{}] with matching labels: {:?}",
            name(&existing_config_map)?,
            config_map.metadata.labels
        );

        // compare hashes and check for changes
        if let Some(labels) = &existing_config_map.metadata.labels {
            if Some(&hash.to_string()) != labels.get(CONFIGMAP_HASH_LABEL) {
                debug!(
                    "ConfigMap [{}] already exists, but differs, updating it!",
                    name(&existing_config_map)?,
                );

                merge_config_maps(&mut existing_config_map, config_map, hash);
                existing_config_map = client.update(&existing_config_map).await?;
            }
        }
        Ok(existing_config_map)
    } else {
        debug!(
            "ConfigMap [{}] not existing, creating it.",
            name(&config_map)?,
        );

        if let Some(labels) = &mut config_map.metadata.labels {
            labels.insert(CONFIGMAP_HASH_LABEL.to_string(), hash.to_string());
        }

        Ok(client.create(&config_map).await?)
    };
}

/// Returns `Ok(Some(ConfigMap))` if a ConfigMap matching the `config_map` labels exists.
/// Returns `Ok(None)` if no ConfigMap matches the `config_map` labels.
///
/// For now we assume that the ConfigMap labels are unique. That means we will return only
/// one matching ConfigMap.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The ConfigMap to create or update.
///
async fn find_config_map(
    client: &Client,
    config_map: &ConfigMap,
) -> OperatorResult<Option<ConfigMap>> {
    let existing_config_maps = client
        .list_with_label_selector::<ConfigMap>(
            Some(&client.default_namespace),
            &LabelSelector {
                match_labels: config_map.metadata.labels.clone(),
                ..LabelSelector::default()
            },
        )
        .await?;

    Ok(filter_config_map(
        existing_config_maps,
        config_map.metadata.labels.as_ref(),
    ))
}

/// Returns `Some(ConfigMap)` if a ConfigMap was found. Otherwise None.
///
/// If the `config_maps` vector size is greater than 1, we log a warning and
/// return the 'newest' ConfigMap, meaning the youngest creation timestamp.
///
/// # Arguments
///
/// - `config_maps` - The ConfigMaps to filter
/// - `labels` - The labels used in the LabelSelector.
///
fn filter_config_map(
    mut config_maps: Vec<ConfigMap>,
    labels: Option<&BTreeMap<String, String>>,
) -> Option<ConfigMap> {
    match config_maps.len() {
        0 => None,
        // This unwrap cannot fail because the vector is not empty
        1 => Some(config_maps.pop().unwrap()),
        _ => {
            // TODO: using the latest? Or error? Not sure what side effects this may have yet.
            warn!(
                "Found {} configmaps for labels {:?}. This is should not happen. Using the ConfigMap \
                with the latest creation timestamp. Please open a ticket.",
                config_maps.len(),
                labels
            );

            config_maps.sort_by(|a, b| {
                a.metadata
                    .creation_timestamp
                    .cmp(&b.metadata.creation_timestamp)
            });
            // This unwrap cannot fail because the vector is not empty
            Some(config_maps.pop().unwrap())
        }
    }
}

/// Checks if the labels contain the following:
/// * `labels::APP_NAME_LABEL`
/// * `labels::APP_INSTANCE_LABEL`
/// * `labels::APP_COMPONENT_LABEL`
/// * `labels::APP_ROLE_GROUP_LABEL`
/// * `labels::APP_MANAGED_BY_LABEL`
/// * `configmap::CONFIGMAP_TYPE_LABEL`
///
/// This is required to uniquely identify the ConfigMap.
///
fn check_labels(cm_name: &str, labels: &BTreeMap<String, String>) -> Result<(), Error> {
    let mut missing_labels = vec![];

    for label in REQUIRED_LABELS.to_vec() {
        if !labels.contains_key(label) {
            missing_labels.push(label);
        }
    }

    if missing_labels.is_empty() {
        Ok(())
    } else {
        Err(Error::ConfigMapMissingLabels {
            name: cm_name.to_string(),
            labels: missing_labels,
        })
    }
}

/// Extract the metadata.name or alternatively metadata.generate_name.
///
/// # Arguments
///
/// - `config_map` - The ConfigMap to extract name or generate_name from.
///
fn name(config_map: &ConfigMap) -> OperatorResult<&str> {
    return match (
        config_map.metadata.name.as_deref(),
        config_map.metadata.generate_name.as_deref(),
    ) {
        (Some(name), Some(_) | None) => Ok(name),
        (None, Some(generate_name)) => Ok(generate_name),
        _ => Err(Error::MissingObjectKey {
            key: "metadata.name",
        }),
    };
}

/// Provides a hash of relevant parts of the ConfigMap in order to react on any changes.
///
/// # Arguments
///
/// - `config_map` - The ConfigMap to hash.
///
fn hash_config_map(config_map: &ConfigMap) -> u64 {
    let mut s = DefaultHasher::new();
    config_map.metadata.labels.hash(&mut s);
    config_map.data.hash(&mut s);
    s.finish()
}

/// Merges the data of the created with the existing ConfigMap.
///
/// # Arguments
///
/// - `existing` - The existing ConfigMap.
/// - `created` - The created ConfigMap.
/// - `hash` - The hash of the created ConfigMap.
///
fn merge_config_maps(existing: &mut ConfigMap, created: ConfigMap, hash: u64) {
    existing.data = created.data;
    existing.metadata.labels = created.metadata.labels;
    // update hash
    if let Some(labels) = &mut existing.metadata.labels {
        labels.insert(CONFIGMAP_HASH_LABEL.to_string(), hash.to_string());
    }
}
