use std::collections::BTreeMap;

use stackable_operator::kvp::Annotations;

use crate::framework::types::kubernetes::{ConfigMapName, SecretName};

/// Creates `restarter.stackable.tech/ignore-configmap.{i}` annotations for each given ConfigMap.
///
/// The restarter uses these annotations to skip restarting Pods when specific ConfigMaps change.
/// Indices start at 0 and are assigned in iteration order, so **do not merge the result with
/// annotations from another call** — duplicate indices would overwrite each other.
pub fn restarter_ignore_configmap_annotations(
    ignored_config_maps: impl IntoIterator<Item = ConfigMapName>,
) -> Annotations {
    let annotation_key_values = ignored_config_maps
        .into_iter()
        .enumerate()
        .map(|(i, config_map_name)| {
            (
                format!("restarter.stackable.tech/ignore-configmap.{i}"),
                config_map_name.to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    Annotations::try_from(annotation_key_values).expect(
        "should contain only valid annotations because the annotation keys are statically \
            defined apart from the index number and the names of ConfigMaps are valid annotation \
            values.",
    )
}

/// Creates `restarter.stackable.tech/ignore-secret.{i}` annotations for each given Secret.
///
/// The restarter uses these annotations to skip restarting Pods when specific Secrets change.
/// Indices start at 0 and are assigned in iteration order, so **do not merge the result with
/// annotations from another call** — duplicate indices would overwrite each other.
pub fn restarter_ignore_secret_annotations(
    ignored_secrets: impl IntoIterator<Item = SecretName>,
) -> Annotations {
    let annotation_key_values = ignored_secrets
        .into_iter()
        .enumerate()
        .map(|(i, secret_name)| {
            (
                format!("restarter.stackable.tech/ignore-secret.{i}"),
                secret_name.to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    Annotations::try_from(annotation_key_values).expect(
        "should contain only valid annotations because the annotation keys are statically \
            defined apart from the index number and the names of Secrets are valid annotation \
            values.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_config_maps_produce_indexed_annotations() {
        let ignored_config_maps = [
            ConfigMapName::from_str_unsafe("first-config"),
            ConfigMapName::from_str_unsafe("second-config"),
            ConfigMapName::from_str_unsafe("third-config"),
        ];

        let actual_annotations = restarter_ignore_configmap_annotations(ignored_config_maps);

        let expected_annotations = BTreeMap::from([
            (
                "restarter.stackable.tech/ignore-configmap.0".to_owned(),
                "first-config".to_owned(),
            ),
            (
                "restarter.stackable.tech/ignore-configmap.1".to_owned(),
                "second-config".to_owned(),
            ),
            (
                "restarter.stackable.tech/ignore-configmap.2".to_owned(),
                "third-config".to_owned(),
            ),
        ]);

        assert_eq!(expected_annotations, actual_annotations.into());
    }

    #[test]
    fn multiple_secrets_produce_indexed_annotations() {
        let ignored_secrets = [
            SecretName::from_str_unsafe("first-secret"),
            SecretName::from_str_unsafe("second-secret"),
            SecretName::from_str_unsafe("third-secret"),
        ];

        let actual_annotations = restarter_ignore_secret_annotations(ignored_secrets);

        let expected_annotations = BTreeMap::from([
            (
                "restarter.stackable.tech/ignore-secret.0".to_owned(),
                "first-secret".to_owned(),
            ),
            (
                "restarter.stackable.tech/ignore-secret.1".to_owned(),
                "second-secret".to_owned(),
            ),
            (
                "restarter.stackable.tech/ignore-secret.2".to_owned(),
                "third-secret".to_owned(),
            ),
        ]);

        assert_eq!(expected_annotations, actual_annotations.into());
    }
}
