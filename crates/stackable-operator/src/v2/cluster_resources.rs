use stackable_operator::{
    cluster_resources::{ClusterResourceApplyStrategy, ClusterResources},
    deep_merger::ObjectOverrides,
    k8s_openapi::api::core::v1::ObjectReference,
};

use super::types::{
    kubernetes::{NamespaceName, Uid},
    operator::{ClusterName, ControllerName, OperatorName, ProductName},
};
use crate::framework::{
    NameIsValidLabelValue, macros::attributed_string_type::MAX_LABEL_VALUE_LENGTH,
};

/// Infallible variant of [`stackable_operator::cluster_resources::ClusterResources::new`]
#[allow(clippy::too_many_arguments)]
pub fn cluster_resources_new<'a>(
    product_name: &ProductName,
    operator_name: &OperatorName,
    controller_name: &ControllerName,
    cluster_name: &ClusterName,
    cluster_namespace: &NamespaceName,
    cluster_uid: &Uid,
    apply_strategy: ClusterResourceApplyStrategy,
    object_overrides: &'a ObjectOverrides,
) -> ClusterResources<'a> {
    // compile-time check
    // ClusterResources::new creates a label value from the given app name by appending
    // `-operator`. For the resulting label value to be valid, it must not exceed
    // MAX_LABEL_VALUE_LENGTH.
    const _: () = assert!(
        ProductName::MAX_LENGTH + "-operator".len() <= MAX_LABEL_VALUE_LENGTH,
        "The string `<cluster_name>-operator` must not exceed the limit of Label names."
    );

    ClusterResources::new(
        &product_name.to_label_value(),
        &operator_name.to_label_value(),
        &controller_name.to_label_value(),
        &ObjectReference {
            name: Some(cluster_name.to_string()),
            namespace: Some(cluster_namespace.to_string()),
            uid: Some(cluster_uid.to_string()),
            ..Default::default()
        },
        apply_strategy,
        object_overrides,
    )
    .expect("ClusterResources should be created because the cluster object reference contains name, namespace and uid.")
}
