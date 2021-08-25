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
//! To have the 'full' name the config maps have to be created before mounting them into pods.
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

/// This is a required label to set in the configmaps to differentiate config maps for e.g.
/// config, data, ids etc.
pub const CONFIGMAP_TYPE_LABEL: &str = "configmap.stackable.tech/type";
/// This label will be set automatically to track the content of the config map.
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

/// This method can be used to build a config map. In order to create, update or delete a config
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
/// - `cluster` - The Kubernetes client.
/// - `name` - The config map name (generate_name).
/// - `namespace` - The config map namespace.
/// - `labels` - The config map labels.
/// - `data` - The config map data.
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
                .build()?,
        )
        .data(data)
        .build()
}

/// This method can be used to ensure a ConfigMap exists and has the specified content.
///
/// If a ConfigMap with the specified name does not exist it will be created.
///
/// Should a ConfigMap with the specified name already exist the content is retrieved and
/// compared with the content from `config_map`, if content differs the existing ConfigMap is
/// updated.
///
/// The config map labels and data are hashed and added as configmap::CONFIGMAP_HASH_LABEL in
/// order to keep track of changes.
///
/// Returns `Ok(ConfigMap)` if created or updated. Otherwise error.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The config map to create or update.
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
        if Some(&hash.to_string())
            != existing_config_map
                .metadata
                .labels
                .get(CONFIGMAP_HASH_LABEL)
        {
            debug!(
                "ConfigMap [{}] already exists, but differs, updating it!",
                name(&existing_config_map)?,
            );

            merge_config_maps(&mut existing_config_map, config_map, hash);
            existing_config_map = client.update(&existing_config_map).await?;
        }

        Ok(existing_config_map)
    } else {
        debug!(
            "ConfigMap [{}] not existing, creating it.",
            name(&config_map)?,
        );

        config_map
            .metadata
            .labels
            .insert(CONFIGMAP_HASH_LABEL.to_string(), hash.to_string());

        Ok(client.create(&config_map).await?)
    };
}

/// Returns `Ok(Some(ConfigMap))` if created or updated. Otherwise Ok(None). Returns Err if
/// anything with listing the configmaps went wrong.
///
/// For now we assume that the config map labels are unique. That means we will return only
/// one matching config map.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The config map to create or update.
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
        &config_map.metadata.labels,
    ))
}

/// Returns `Some(ConfigMap)` if a config map was found. Otherwise None.
///
/// If the `config_maps` vector size is greater than 1, we log a warning and
/// return the 'newest' config map, meaning the youngest creation timestamp.
///
/// # Arguments
///
/// - `config_maps` - The config maps to filter
/// - `labels` - The labels used in the LabelSelector.
///
fn filter_config_map(
    mut config_maps: Vec<ConfigMap>,
    labels: &BTreeMap<String, String>,
) -> Option<ConfigMap> {
    match config_maps.len() {
        0 => None,
        // This unwrap cannot fail because the vector is not empty
        1 => Some(config_maps.pop().unwrap()),
        _ => {
            // TODO: using the latest? Or error? Not sure what side effects this may have yet.
            warn!(
                "Found {} configmaps for labels {:?}. This is should not happen. Using the config map \
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
/// This is required to uniquely identify the config map.
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
/// - `config_map` - The config map to extract name or generate_name from.
///
fn name(config_map: &ConfigMap) -> OperatorResult<&str> {
    return match (
        config_map.metadata.name.as_deref(),
        config_map.metadata.generate_name.as_deref(),
    ) {
        (Some(name), Some(_)) | (Some(name), None) => Ok(name),
        (None, Some(generate_name)) => Ok(generate_name),
        _ => Err(Error::MissingObjectKey {
            key: "metadata.name",
        }),
    };
}

/// Provides a hash of relevant parts of the config map in order to react on any changes.
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

/// Merges the data of the created with the existing config map.
///
/// # Arguments
///
/// - `existing` - The existing config map.
/// - `created` - The created config map.
/// - `hash` - The hash of the created config map.
///
fn merge_config_maps(existing: &mut ConfigMap, created: ConfigMap, hash: u64) {
    existing.data = created.data;
    existing.metadata.labels = created.metadata.labels;
    // update hash
    existing
        .metadata
        .labels
        .insert(CONFIGMAP_HASH_LABEL.to_string(), hash.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels;
    use chrono::{Duration, Utc};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use kube::api::ObjectMeta;
    use rstest::*;

    #[test]
    fn test_filter_config_maps_multiple() {
        let time_old = Time(Utc::now());
        let time_old_2 = Time(Utc::now() + Duration::minutes(10));
        let time_new = Time(Utc::now() + Duration::days(1));

        let config_maps = vec![
            ConfigMap {
                metadata: ObjectMeta {
                    creation_timestamp: Some(time_old),
                    ..Default::default()
                },
                ..Default::default()
            },
            ConfigMap {
                metadata: ObjectMeta {
                    creation_timestamp: Some(time_new.clone()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ConfigMap {
                metadata: ObjectMeta {
                    creation_timestamp: Some(time_old_2),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let filtered = filter_config_map(config_maps, &BTreeMap::new()).unwrap();
        assert_eq!(filtered.metadata.creation_timestamp, Some(time_new));
    }

    #[rstest]
    #[case(vec![], false)]
    #[case(vec![ConfigMap::default()], true)]
    fn test_filter_config_maps_single(#[case] config_maps: Vec<ConfigMap>, #[case] expected: bool) {
        let filtered = filter_config_map(config_maps, &BTreeMap::new());
        assert_eq!(filtered.is_some(), expected);
    }

    #[test]
    fn test_check_labels() {
        let cm_name = "test";
        let mut test_labels = BTreeMap::new();
        test_labels.insert(labels::APP_NAME_LABEL.to_string(), "test".to_string());
        test_labels.insert(labels::APP_INSTANCE_LABEL.to_string(), "test".to_string());

        assert!(check_labels(cm_name, &test_labels).is_err());

        test_labels.insert(labels::APP_COMPONENT_LABEL.to_string(), "test".to_string());
        test_labels.insert(labels::APP_ROLE_GROUP_LABEL.to_string(), "test".to_string());
        test_labels.insert(labels::APP_MANAGED_BY_LABEL.to_string(), "test".to_string());
        test_labels.insert(CONFIGMAP_TYPE_LABEL.to_string(), "test".to_string());

        assert!(check_labels(cm_name, &test_labels).is_ok());
    }

    #[rstest]
    #[case(Some("name".to_string()), None, "name")]
    #[case(Some("name".to_string()), Some("generated-name-".to_string()), "name")]
    #[case(None, Some("generated-name-".to_string()), "generated-name-")]
    fn test_name_ok(
        #[case] normal_name: Option<String>,
        #[case] generate_name: Option<String>,
        #[case] expected: &str,
    ) {
        let mut cm = ConfigMap::default();
        cm.metadata.name = normal_name;
        cm.metadata.generate_name = generate_name;

        assert_eq!(name(&cm).unwrap(), expected.to_string())
    }

    #[rstest]
    #[case(None, None)]
    fn test_name_err(#[case] normal_name: Option<String>, #[case] generate_name: Option<String>) {
        let mut cm = ConfigMap::default();
        cm.metadata.name = normal_name;
        cm.metadata.generate_name = generate_name;

        assert!(name(&cm).is_err());
    }

    #[rstest]
    #[case(vec![], vec![], vec![], vec![], true)]
    #[case(vec!["labels".to_string()], vec![], vec!["labels".to_string()], vec![], true)]
    #[case(vec![], vec!["data".to_string()], vec![], vec!["data".to_string()], true)]
    #[case(vec!["labels".to_string()], vec!["data".to_string()], vec!["labels".to_string()], vec!["data".to_string()], true)]
    #[case(vec!["labels".to_string()], vec!["data".to_string()], vec!["other_labels".to_string()], vec!["data".to_string()], false)]
    #[case(vec!["labels".to_string()], vec!["data".to_string()], vec!["other_labels".to_string()], vec!["other_data".to_string()], false)]
    fn test_hash_config_map(
        #[case] labels_1: Vec<String>,
        #[case] data_1: Vec<String>,
        #[case] labels_2: Vec<String>,
        #[case] data_2: Vec<String>,
        #[case] expected: bool,
    ) {
        let mut cm_1 = ConfigMap::default();
        let mut cm_2 = ConfigMap::default();
        let mut cm_labels_1 = BTreeMap::new();
        let mut cm_data_1 = BTreeMap::new();
        let mut cm_labels_2 = BTreeMap::new();
        let mut cm_data_2 = BTreeMap::new();

        for label in labels_1 {
            cm_labels_1.insert(label.clone(), label.clone());
        }

        for data in data_1 {
            cm_data_1.insert(data.clone(), data.clone());
        }

        for label in labels_2 {
            cm_labels_2.insert(label.clone(), label.clone());
        }

        for data in data_2 {
            cm_data_2.insert(data.clone(), data.clone());
        }

        cm_1.metadata.labels = cm_labels_1;
        cm_1.data = cm_data_1;

        cm_2.metadata.labels = cm_labels_2;
        cm_2.data = cm_data_2;

        assert_eq!(hash_config_map(&cm_1) == hash_config_map(&cm_2), expected);
    }

    #[test]
    fn test_merge_config_maps() {
        let mut cm_found = ConfigMap::default();
        let mut cm_new = ConfigMap::default();
        let mut cm_new_labels = BTreeMap::new();
        let mut cm_new_data = BTreeMap::new();

        cm_new_labels.insert("new_label_key".to_string(), "new_label_value".to_string());
        cm_new_data.insert("new_data_key".to_string(), "new_data_value".to_string());

        let hash = hash_config_map(&cm_new);
        cm_new_labels.insert(CONFIGMAP_HASH_LABEL.to_string(), hash.to_string());

        cm_new.metadata.labels = cm_new_labels.clone();
        cm_new.data = cm_new_data.clone();

        merge_config_maps(&mut cm_found, cm_new, hash);

        assert_eq!(cm_new_labels, cm_found.metadata.labels);
        assert_eq!(cm_new_data, cm_found.data);
        assert_eq!(
            Some(&hash.to_string()),
            cm_found.metadata.labels.get(CONFIGMAP_HASH_LABEL)
        );
    }
}
