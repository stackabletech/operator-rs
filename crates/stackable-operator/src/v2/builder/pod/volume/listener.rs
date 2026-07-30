use k8s_openapi::{
    api::core::v1::{
        EphemeralVolumeSource, PersistentVolumeClaim, PersistentVolumeClaimSpec,
        PersistentVolumeClaimTemplate, VolumeResourceRequirements,
    },
    apimachinery::pkg::api::resource::Quantity,
};
use snafu::{ResultExt, Snafu};

use crate::{
    builder::meta::ObjectMetaBuilder,
    kvp::{Annotation, AnnotationError, LabelError, Labels},
    v2::types::kubernetes::{ListenerClassName, ListenerName, PersistentVolumeClaimName},
};

/// Reference to a listener class or listener name
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerReference {
    ListenerClass(ListenerClassName),
    Listener(ListenerName),
}

impl ListenerReference {
    /// Return the key and value for a Kubernetes object annotation
    fn to_annotation(&self) -> Result<Annotation, AnnotationError> {
        match self {
            Self::ListenerClass(class) => {
                Annotation::try_from(("listeners.stackable.tech/listener-class", class.to_string()))
            }
            Self::Listener(name) => {
                Annotation::try_from(("listeners.stackable.tech/listener-name", name.to_string()))
            }
        }
    }
}

// NOTE (Techassi): We might want to think about these names and how long they
// are getting.
#[derive(Debug, PartialEq, Eq, Snafu)]
pub enum ListenerOperatorVolumeSourceBuilderError {
    #[snafu(display("failed to convert listener reference into Kubernetes annotation"))]
    ListenerReferenceAnnotation { source: AnnotationError },
    #[snafu(display("invalid recommended labels"))]
    RecommendedLabels { source: LabelError },
}

/// Builder for an [`EphemeralVolumeSource`] containing the listener configuration
///
/// # Example
///
/// ```
/// # use k8s_openapi::api::core::v1::Volume;
/// # use stackable_operator::builder::pod::volume::ListenerReference;
/// # use stackable_operator::builder::pod::volume::ListenerOperatorVolumeSourceBuilder;
/// # use stackable_operator::builder::pod::PodBuilder;
/// # use stackable_operator::kvp::Labels;
/// # use k8s_openapi::{
/// #     apimachinery::pkg::apis::meta::v1::ObjectMeta,
/// # };
/// # use std::collections::BTreeMap;
/// let mut pod_builder = PodBuilder::new();
///
/// let labels: Labels = Labels::try_from(BTreeMap::<String, String>::new()).unwrap();
///
/// let volume_source = ListenerOperatorVolumeSourceBuilder::new(
///     &ListenerReference::ListenerClass("nodeport".into()),
///     &labels,
/// )
/// .build_ephemeral()
/// .unwrap();
///
/// pod_builder.add_volume(Volume {
///     name: "listener".to_string(),
///     ephemeral: Some(volume_source),
///     ..Volume::default()
/// });
///
/// // There is also a shortcut for the code above:
/// pod_builder.add_listener_volume_by_listener_class("listener", "nodeport", &labels);
/// ```
#[derive(Clone, Debug)]
pub struct ListenerOperatorVolumeSourceBuilder {
    listener_reference: ListenerReference,
    labels: Labels,
}

impl ListenerOperatorVolumeSourceBuilder {
    /// Create a builder for the given listener class or listener name
    pub fn new(listener_reference: &ListenerReference, labels: &Labels) -> Self {
        Self {
            listener_reference: listener_reference.to_owned(),
            labels: labels.to_owned(),
        }
    }

    #[deprecated(note = "renamed to `build_ephemeral`", since = "0.61.1")]
    pub fn build(&self) -> Result<EphemeralVolumeSource, ListenerOperatorVolumeSourceBuilderError> {
        self.build_ephemeral()
    }

    /// Build an [`EphemeralVolumeSource`] from the builder.
    pub fn build_ephemeral(
        &self,
    ) -> Result<EphemeralVolumeSource, ListenerOperatorVolumeSourceBuilderError> {
        let listener_reference_annotation = self
            .listener_reference
            .to_annotation()
            .context(ListenerReferenceAnnotationSnafu)?;

        Ok(EphemeralVolumeSource {
            volume_claim_template: Some(PersistentVolumeClaimTemplate {
                metadata: Some(
                    ObjectMetaBuilder::new()
                        .with_annotation(listener_reference_annotation)
                        .with_labels(self.labels.clone())
                        .build(),
                ),
                spec: Self::spec(),
            }),
        })
    }

    /// Build a [`PersistentVolumeClaim`] from the builder.
    pub fn build_pvc(&self, name: &PersistentVolumeClaimName) -> PersistentVolumeClaim {
        let listener_reference_annotation = self
            .listener_reference
            .to_annotation()
            .expect(
                "should return a PersistentVolumeClaim, because the only check is that \
                listener_reference is a valid annotation value and there are no restrictions on single \
                annotation values",
            );

        PersistentVolumeClaim {
            metadata: ObjectMetaBuilder::new()
                .name(name.to_string())
                .with_annotation(listener_reference_annotation)
                .with_labels(self.labels.clone())
                .build(),
            spec: Some(Self::spec()),
            ..Default::default()
        }
    }

    fn spec() -> PersistentVolumeClaimSpec {
        PersistentVolumeClaimSpec {
            storage_class_name: Some("listeners.stackable.tech".to_string()),
            resources: Some(VolumeResourceRequirements {
                requests: Some([("storage".to_string(), Quantity("1".to_string()))].into()),
                ..Default::default()
            }),
            access_modes: Some(vec!["ReadWriteMany".to_string()]),
            ..PersistentVolumeClaimSpec::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    use super::*;

    #[test]
    fn listener_operator_volume_source_builder() {
        let labels: Labels = Labels::try_from(BTreeMap::<String, String>::new()).unwrap();

        let builder = ListenerOperatorVolumeSourceBuilder::new(
            &ListenerReference::ListenerClass(ListenerClassName::from_str_unsafe("public")),
            &labels,
        );

        let volume_source = builder.build_ephemeral().unwrap();

        let volume_claim_template = volume_source.volume_claim_template;
        let annotations = volume_claim_template
            .as_ref()
            .and_then(|template| template.metadata.as_ref())
            .and_then(|metadata| metadata.annotations.as_ref())
            .cloned()
            .unwrap_or_default();
        let spec = volume_claim_template.unwrap_or_default().spec;
        let access_modes = spec.access_modes.unwrap_or_default();
        let requests = spec
            .resources
            .and_then(|resources| resources.requests)
            .unwrap_or_default();

        assert_eq!(1, annotations.len());
        assert_eq!(
            Some((
                &"listeners.stackable.tech/listener-class".to_string(),
                &"public".to_string()
            )),
            annotations.iter().next()
        );
        assert_eq!(
            Some("listeners.stackable.tech".to_string()),
            spec.storage_class_name
        );
        assert_eq!(1, access_modes.len());
        assert_eq!(Some(&"ReadWriteMany".to_string()), access_modes.first());
        assert_eq!(1, requests.len());
        assert_eq!(
            Some((&"storage".to_string(), &Quantity("1".into()))),
            requests.iter().next()
        );
    }
}
