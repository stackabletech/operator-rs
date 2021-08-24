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
use tracing::{debug, info, warn};

/// This is a required label to set in the configmaps to differentiate config maps for e.g.
/// config, data, ids etc.
pub const CONFIGMAP_TYPE_LABEL: &str = "configmap.stackable.tech/type";
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
/// The labels and data are hashed and added as configmap::CONFIGMAP_HASH_LABEL.
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
            name(&existing_config_map),
            config_map.metadata.labels
        );

        // compare hashes and check for changes
        if Some(&hash.to_string())
            != existing_config_map
                .metadata
                .labels
                .get(CONFIGMAP_HASH_LABEL)
        {
            info!(
                "ConfigMap [{}] already exists, but differs, updating it!",
                name(&existing_config_map),
            );

            merge_config_maps(&mut existing_config_map, config_map, hash);
            existing_config_map = client.update(&existing_config_map).await?;
        }

        Ok(existing_config_map)
    } else {
        debug!(
            "ConfigMap [{}] not existing, creating it.",
            name(&config_map),
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
/// If for some reason multiple config maps match the label selector, we log a warning and
/// return the 'newest' config map, meaning the youngest creation timestamp.
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
    let mut existing_config_maps = client
        .list_with_label_selector::<ConfigMap>(
            Some(&client.default_namespace),
            &LabelSelector {
                match_labels: config_map.metadata.labels.clone(),
                ..LabelSelector::default()
            },
        )
        .await?;

    return match existing_config_maps.len() {
        0 => Ok(None),
        // This unwrap cannot fail because the vector is not empty
        1 => Ok(Some(existing_config_maps.pop().unwrap())),
        _ => {
            warn!(
                "Found {} configmaps for labels {:?}. This is should not happen. Using the config map \
                with the latest creation timestamp. Please open a ticket.",
                existing_config_maps.len(),
                config_map.metadata.labels
            );

            existing_config_maps.sort_by(|a, b| {
                b.metadata
                    .creation_timestamp
                    .cmp(&a.metadata.creation_timestamp)
            });
            // This unwrap cannot fail because the vector is not empty
            Ok(Some(existing_config_maps.pop().unwrap()))
        }
    };
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
fn name(config_map: &ConfigMap) -> &str {
    return match (
        config_map.metadata.name.as_deref(),
        config_map.metadata.generate_name.as_deref(),
    ) {
        (Some(name), Some(_)) | (Some(name), None) => name,
        (None, Some(generate_name)) => generate_name,
        _ => "<no-name-found>",
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
