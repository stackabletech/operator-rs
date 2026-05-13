use stackable_operator::{
    builder::pod::volume::ListenerOperatorVolumeSourceBuilder,
    k8s_openapi::api::core::v1::PersistentVolumeClaim, kvp::Labels,
};

use crate::framework::types::kubernetes::{
    ListenerClassName, ListenerName, PersistentVolumeClaimName,
};

/// Infallible variant of [`stackable_operator::builder::pod::volume::ListenerReference`]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerReference {
    ListenerClass(ListenerClassName),
    Listener(ListenerName),
}

impl From<&ListenerReference> for stackable_operator::builder::pod::volume::ListenerReference {
    fn from(value: &ListenerReference) -> Self {
        match value {
            ListenerReference::ListenerClass(listener_class_name) => {
                stackable_operator::builder::pod::volume::ListenerReference::ListenerClass(
                    listener_class_name.to_string(),
                )
            }
            ListenerReference::Listener(listener_name) => {
                stackable_operator::builder::pod::volume::ListenerReference::ListenerName(
                    listener_name.to_string(),
                )
            }
        }
    }
}

/// Infallible variant of
/// [`stackable_operator::builder::pod::volume::ListenerOperatorVolumeSourceBuilder::build_pvc`]
pub fn listener_operator_volume_source_builder_build_pvc(
    listener_reference: &ListenerReference,
    labels: &Labels,
    pvc_name: &PersistentVolumeClaimName,
) -> PersistentVolumeClaim {
    ListenerOperatorVolumeSourceBuilder::new(&listener_reference.into(), labels)
        .build_pvc(pvc_name.to_string())
        .expect(
            "should return a PersistentVolumeClaim, because the only check is that \
            listener_reference is a valid annotation value and there are no restrictions on single \
            annotation values",
        )
}
