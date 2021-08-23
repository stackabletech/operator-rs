use crate::builder::{ConfigMapBuilder, ObjectMetaBuilder};
use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::labels;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::Resource;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use tracing::info;

/// This is a required label to set in the configmaps to differentiate config maps for e.g.
/// config, ids etc.
pub const CM_TYPE_LABEL: &str = "configmap.stackable.tech/type";

lazy_static! {
    static ref REQUIRED_LABELS: Vec<&'static str> = {
        vec![
            labels::APP_NAME_LABEL,
            labels::APP_INSTANCE_LABEL,
            labels::APP_COMPONENT_LABEL,
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

/// Checks if the labels contain the following:
/// * `labels::APP_NAME_LABEL`
/// * `labels::APP_INSTANCE_LABEL`
/// * `labels::APP_COMPONENT_LABEL`
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

/// This method can be used to ensure a ConfigMap exists and has the specified content.
///
/// If a ConfigMap with the specified name does not exist it will be created.
///
/// Should a ConfigMap with the specified name already exist the content is retrieved and
/// compared with the content from `config_map`, if content differs the existing ConfigMap is
/// updated.
///
/// Returns `Ok(())` if created or updated. Otherwise error.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The config map to create or update.
///
pub async fn create_config_map(client: &Client, config_map: ConfigMap) -> OperatorResult<()> {
    let existing_config_maps = client
        .list_with_label_selector::<ConfigMap>(
            Some(&client.default_namespace),
            &LabelSelector {
                match_labels: config_map.metadata.labels.clone(),
                ..LabelSelector::default()
            },
        )
        .await?;

    info!(
        "Found {} configmap(s) with matching labels: {:?}",
        existing_config_maps.len(),
        config_map.metadata.labels
    );

    let cm_name = match config_map.metadata.generate_name.as_deref() {
        None => return Err(Error::ConfigMapMissingGenerateName),
        Some(name) => name,
    };

    if existing_config_maps.is_empty() {
        // nothing there yet, we need to create
        info!("ConfigMap [{}] not existing, creating it.", cm_name);
        client.create(&config_map).await?;
    } else if existing_config_maps.len() == 1 {
        if existing_config_maps.get(0).unwrap().data == config_map.data {
            info!(
                "ConfigMap [{}] already exists with identical data, skipping creation!",
                cm_name
            );
        } else {
            info!(
                "ConfigMap [{}] already exists, but differs, updating it!",
                cm_name
            );
            client.update(&config_map).await?;
        }
    }

    Ok(())
}
