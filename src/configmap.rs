use crate::builder::{ConfigMapBuilder, ObjectMetaBuilder};
use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::labels;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::Resource;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use tracing::{info, warn};

/// This is a required label to set in the configmaps to differentiate config maps for e.g.
/// config, data, ids etc.
pub const CM_TYPE_LABEL: &str = "configmap.stackable.tech/type";

lazy_static! {
    static ref REQUIRED_LABELS: Vec<&'static str> = {
        vec![
            labels::APP_NAME_LABEL,
            labels::APP_INSTANCE_LABEL,
            labels::APP_COMPONENT_LABEL,
            labels::APP_ROLE_GROUP_LABEL,
            labels::APP_MANAGED_BY_LABEL,
            CM_TYPE_LABEL,
        ]
    };
}

/// This method can be used to build a config map. In order to create, update or delete a config
/// map, it has to be uniquely identifiable via a LabelSelector. That is why the `labels` must
/// contain at least the following labels:
/// * `labels::APP_NAME_LABEL`
/// * `labels::APP_INSTANCE_LABEL`
/// * `labels::APP_COMPONENT_LABEL`
/// * `labels::APP_ROLE_GROUP_LABEL`
/// * `labels::APP_MANAGED_BY_LABEL`
/// * `config_map_utils::CM_TYPE_LABEL`
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
                .generate_name(name.clone())
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
    config_map: ConfigMap,
) -> OperatorResult<ConfigMap> {
    return if let Some(mut cm) = list_config_map(client, &config_map).await? {
        info!(
            "Found a configmap [{}] with matching labels: {:?}",
            name(&config_map),
            config_map.metadata.labels
        );

        if cm.data != config_map.data {
            cm = client.update(&config_map).await?;
            info!(
                "ConfigMap [{}] already exists, but differs, updating it!",
                name(&config_map),
            );
        }

        Ok(cm)
    } else {
        info!(
            "ConfigMap [{}] not existing, creating it.",
            name(&config_map),
        );
        Ok(client.create(&config_map).await?)
    };
}

/// Checks if the labels contain the following:
/// * `labels::APP_NAME_LABEL`
/// * `labels::APP_INSTANCE_LABEL`
/// * `labels::APP_COMPONENT_LABEL`
/// * `labels::APP_ROLE_GROUP_LABEL`
/// * `labels::APP_MANAGED_BY_LABEL`
/// * `config_map_utils::CM_TYPE_LABEL`
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

    return if missing_labels.is_empty() {
        Ok(())
    } else {
        Err(Error::ConfigMapMissingLabels {
            name: cm_name.to_string(),
            labels: missing_labels,
        })
    };
}

/// Returns `Ok(Some(ConfigMap))` if created or updated. Otherwise Ok(None). Returns Err if
/// anything with listing the configmaps went wrong.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The config map to create or update.
///
async fn list_config_map(
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
