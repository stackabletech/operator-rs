use crate::builder::{ConfigMapBuilder, ObjectMetaBuilder};
use crate::client::Client;
use crate::error::{Error, OperatorResult};
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::Resource;
use std::collections::BTreeMap;
use tracing::{debug, info};

/// This method can be used to build a config map.
///
/// The labels must contain:
/// *
///
/// # Arguments
///
/// - `cluster` - The Kubernetes client.
/// - `name` - The config map to create or update.
/// - `namespace` - The config map to create or update.
/// - `labels` - The config map to create or update.
/// - `data` - The config map to create or update.
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
/// Returns `Ok(())` if created or updated. Otherwise error.
///
/// # Arguments
///
/// - `client` - The Kubernetes client.
/// - `config_map` - The config map to create or update.
///
pub async fn create_config_map(client: &Client, config_map: ConfigMap) -> OperatorResult<()> {
    let existing_config_maps: Vec<ConfigMap> = client
        .list_with_label_selector(
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

    let cm_name = match config_map.metadata.name.as_deref() {
        None => return Err(Error::InvalidName { errors: vec![] }),
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
