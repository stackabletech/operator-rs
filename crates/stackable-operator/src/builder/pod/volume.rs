use k8s_openapi::{
    api::core::v1::{
        CSIVolumeSource, ConfigMapVolumeSource, DownwardAPIVolumeSource, EmptyDirVolumeSource,
        EphemeralVolumeSource, HostPathVolumeSource, PersistentVolumeClaim,
        PersistentVolumeClaimSpec, PersistentVolumeClaimTemplate,
        PersistentVolumeClaimVolumeSource, ProjectedVolumeSource, SecretVolumeSource, Volume,
        VolumeMount, VolumeResourceRequirements,
    },
    apimachinery::pkg::api::resource::Quantity,
};

use snafu::{ResultExt, Snafu};
use tracing::warn;

use crate::{
    builder::meta::ObjectMetaBuilder,
    kvp::{Annotation, AnnotationError, Annotations, LabelError, Labels},
};

/// A builder to build [`Volume`] objects. May only contain one `volume_source`
/// at a time. E.g. a call like `secret` after `empty_dir` will overwrite the
/// `empty_dir`.
#[derive(Clone, Default)]
pub struct VolumeBuilder {
    name: String,
    volume_source: VolumeSource,
}

#[derive(Clone)]
pub enum VolumeSource {
    ConfigMap(ConfigMapVolumeSource),
    DownwardApi(DownwardAPIVolumeSource),
    EmptyDir(EmptyDirVolumeSource),
    HostPath(HostPathVolumeSource),
    PersistentVolumeClaim(PersistentVolumeClaimVolumeSource),
    Projected(ProjectedVolumeSource),
    Secret(SecretVolumeSource),
    Csi(CSIVolumeSource),
    Ephemeral(Box<EphemeralVolumeSource>),
}

impl Default for VolumeSource {
    fn default() -> Self {
        Self::EmptyDir(EmptyDirVolumeSource {
            ..EmptyDirVolumeSource::default()
        })
    }
}

impl VolumeBuilder {
    pub fn new(name: impl Into<String>) -> VolumeBuilder {
        VolumeBuilder {
            name: name.into(),
            ..VolumeBuilder::default()
        }
    }

    pub fn config_map(&mut self, config_map: impl Into<ConfigMapVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::ConfigMap(config_map.into());
        self
    }

    pub fn with_config_map(&mut self, name: impl Into<String>) -> &mut Self {
        self.volume_source = VolumeSource::ConfigMap(ConfigMapVolumeSource {
            name: name.into(),
            ..ConfigMapVolumeSource::default()
        });
        self
    }

    pub fn downward_api(&mut self, downward_api: impl Into<DownwardAPIVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::DownwardApi(downward_api.into());
        self
    }

    pub fn empty_dir(&mut self, empty_dir: impl Into<EmptyDirVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::EmptyDir(empty_dir.into());
        self
    }

    pub fn with_empty_dir(
        &mut self,
        medium: Option<impl Into<String>>,
        quantity: Option<Quantity>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::EmptyDir(EmptyDirVolumeSource {
            medium: medium.map(|m| m.into()),
            size_limit: quantity,
        });
        self
    }

    pub fn host_path(&mut self, host_path: impl Into<HostPathVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::HostPath(host_path.into());
        self
    }

    pub fn with_host_path(
        &mut self,
        path: impl Into<String>,
        type_: Option<impl Into<String>>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::HostPath(HostPathVolumeSource {
            path: path.into(),
            type_: type_.map(|t| t.into()),
        });
        self
    }

    pub fn persistent_volume_claim(
        &mut self,
        persistent_volume_claim: impl Into<PersistentVolumeClaimVolumeSource>,
    ) -> &mut Self {
        self.volume_source = VolumeSource::PersistentVolumeClaim(persistent_volume_claim.into());
        self
    }

    pub fn with_persistent_volume_claim(
        &mut self,
        claim_name: impl Into<String>,
        read_only: bool,
    ) -> &mut Self {
        self.volume_source =
            VolumeSource::PersistentVolumeClaim(PersistentVolumeClaimVolumeSource {
                claim_name: claim_name.into(),
                read_only: Some(read_only),
            });
        self
    }

    pub fn projected(&mut self, projected: impl Into<ProjectedVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Projected(projected.into());
        self
    }

    pub fn secret(&mut self, secret: impl Into<SecretVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Secret(secret.into());
        self
    }

    pub fn with_secret(&mut self, secret_name: impl Into<String>, optional: bool) -> &mut Self {
        self.volume_source = VolumeSource::Secret(SecretVolumeSource {
            optional: Some(optional),
            secret_name: Some(secret_name.into()),
            ..SecretVolumeSource::default()
        });
        self
    }

    pub fn csi(&mut self, csi: impl Into<CSIVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Csi(csi.into());
        self
    }

    pub fn ephemeral(&mut self, ephemeral: impl Into<EphemeralVolumeSource>) -> &mut Self {
        self.volume_source = VolumeSource::Ephemeral(Box::new(ephemeral.into()));
        self
    }

    /// Consumes the Builder and returns a constructed Volume
    pub fn build(&self) -> Volume {
        let name = self.name.clone();
        match &self.volume_source {
            VolumeSource::ConfigMap(cm) => Volume {
                name,
                config_map: Some(cm.clone()),
                ..Volume::default()
            },
            VolumeSource::DownwardApi(downward_api) => Volume {
                name,
                downward_api: Some(downward_api.clone()),
                ..Volume::default()
            },
            VolumeSource::EmptyDir(empty_dir) => Volume {
                name,
                empty_dir: Some(empty_dir.clone()),
                ..Volume::default()
            },
            VolumeSource::HostPath(host_path) => Volume {
                name,
                host_path: Some(host_path.clone()),
                ..Volume::default()
            },
            VolumeSource::PersistentVolumeClaim(pvc) => Volume {
                name,
                persistent_volume_claim: Some(pvc.clone()),
                ..Volume::default()
            },
            VolumeSource::Projected(projected) => Volume {
                name,
                projected: Some(projected.clone()),
                ..Volume::default()
            },
            VolumeSource::Secret(secret) => Volume {
                name,
                secret: Some(secret.clone()),
                ..Volume::default()
            },
            VolumeSource::Csi(csi) => Volume {
                name,
                csi: Some(csi.clone()),
                ..Volume::default()
            },
            VolumeSource::Ephemeral(ephemeral) => Volume {
                name,
                ephemeral: Some((**ephemeral).clone()),
                ..Volume::default()
            },
        }
    }
}

/// A builder to build [`VolumeMount`] objects.
#[derive(Clone, Default)]
pub struct VolumeMountBuilder {
    mount_path: String,
    mount_propagation: Option<String>,
    name: String,
    read_only: Option<bool>,
    sub_path: Option<String>,
    sub_path_expr: Option<String>,
}

impl VolumeMountBuilder {
    pub fn new(name: impl Into<String>, mount_path: impl Into<String>) -> VolumeMountBuilder {
        VolumeMountBuilder {
            mount_path: mount_path.into(),
            name: name.into(),
            ..VolumeMountBuilder::default()
        }
    }

    pub fn read_only(&mut self, read_only: bool) -> &mut Self {
        self.read_only = Some(read_only);
        self
    }

    pub fn mount_propagation(&mut self, mount_propagation: impl Into<String>) -> &mut Self {
        self.mount_propagation = Some(mount_propagation.into());
        self
    }

    pub fn sub_path(&mut self, sub_path: impl Into<String>) -> &mut Self {
        self.sub_path = Some(sub_path.into());
        self
    }

    pub fn sub_path_expr(&mut self, sub_path_expr: impl Into<String>) -> &mut Self {
        self.sub_path_expr = Some(sub_path_expr.into());
        self
    }

    /// Consumes the Builder and returns a constructed VolumeMount
    pub fn build(&self) -> VolumeMount {
        VolumeMount {
            mount_path: self.mount_path.clone(),
            mount_propagation: self.mount_propagation.clone(),
            name: self.name.clone(),
            read_only: self.read_only,
            sub_path: self.sub_path.clone(),
            sub_path_expr: self.sub_path_expr.clone(),
            // This attribute is supported starting with Kubernetes 1.30.
            // Because we support older Kubernetes versions as well, we can not
            // use it for now, as we would not work on older Kubernetes clusters.
            recursive_read_only: None,
        }
    }
}

#[derive(Debug, PartialEq, Snafu)]
pub enum SecretOperatorVolumeSourceBuilderError {
    #[snafu(display("failed to parse secret operator volume annotation"))]
    ParseAnnotation { source: AnnotationError },
}

#[derive(Clone)]
pub struct SecretOperatorVolumeSourceBuilder {
    secret_class: String,
    scopes: Vec<SecretOperatorVolumeScope>,
    format: Option<SecretFormat>,
    kerberos_service_names: Vec<String>,
    tls_pkcs12_password: Option<String>,
}

impl SecretOperatorVolumeSourceBuilder {
    pub fn new(secret_class: impl Into<String>) -> Self {
        Self {
            secret_class: secret_class.into(),
            scopes: Vec::new(),
            format: None,
            kerberos_service_names: Vec::new(),
            tls_pkcs12_password: None,
        }
    }

    pub fn with_node_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Node);
        self
    }

    pub fn with_pod_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Pod);
        self
    }

    pub fn with_service_scope(&mut self, name: impl Into<String>) -> &mut Self {
        self.scopes
            .push(SecretOperatorVolumeScope::Service { name: name.into() });
        self
    }

    pub fn with_listener_volume_scope(&mut self, name: impl Into<String>) -> &mut Self {
        self.scopes
            .push(SecretOperatorVolumeScope::ListenerVolume { name: name.into() });
        self
    }

    pub fn with_format(&mut self, format: SecretFormat) -> &mut Self {
        self.format = Some(format);
        self
    }

    pub fn with_kerberos_service_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.kerberos_service_names.push(name.into());
        self
    }

    pub fn with_tls_pkcs12_password(&mut self, password: impl Into<String>) -> &mut Self {
        self.tls_pkcs12_password = Some(password.into());
        self
    }

    pub fn build(&self) -> Result<EphemeralVolumeSource, SecretOperatorVolumeSourceBuilderError> {
        let mut annotations = Annotations::new();

        annotations
            .insert(Annotation::secret_class(&self.secret_class).context(ParseAnnotationSnafu)?);

        if !self.scopes.is_empty() {
            annotations
                .insert(Annotation::secret_scope(&self.scopes).context(ParseAnnotationSnafu)?);
        }

        if let Some(format) = &self.format {
            annotations
                .insert(Annotation::secret_format(format.as_ref()).context(ParseAnnotationSnafu)?);
        }

        if !self.kerberos_service_names.is_empty() {
            annotations.insert(
                Annotation::kerberos_service_names(&self.kerberos_service_names)
                    .context(ParseAnnotationSnafu)?,
            );
        }

        if let Some(password) = &self.tls_pkcs12_password {
            // The `tls_pkcs12_password` is only used for PKCS12 stores.
            if Some(SecretFormat::TlsPkcs12) != self.format {
                warn!(format.actual = ?self.format, format.expected = ?Some(SecretFormat::TlsPkcs12), "A TLS PKCS12 password was set but ignored because another format was requested")
            } else {
                annotations.insert(
                    Annotation::tls_pkcs12_password(password).context(ParseAnnotationSnafu)?,
                );
            }
        }

        Ok(EphemeralVolumeSource {
            volume_claim_template: Some(PersistentVolumeClaimTemplate {
                metadata: Some(ObjectMetaBuilder::new().annotations(annotations).build()),
                spec: PersistentVolumeClaimSpec {
                    storage_class_name: Some("secrets.stackable.tech".to_string()),
                    resources: Some(VolumeResourceRequirements {
                        requests: Some([("storage".to_string(), Quantity("1".to_string()))].into()),
                        ..Default::default()
                    }),
                    access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                    ..PersistentVolumeClaimSpec::default()
                },
            }),
        })
    }
}

/// A [secret format](https://docs.stackable.tech/home/stable/secret-operator/secretclass.html#format) known by secret-operator.
///
/// This must either match or be convertible from the corresponding secret class, or provisioning the volume will fail.
#[derive(Clone, Debug, PartialEq, Eq, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum SecretFormat {
    /// A TLS certificate formatted as a PEM triple (`ca.crt`, `tls.crt`, `tls.key`) according to Kubernetes conventions.
    TlsPem,
    /// A TLS certificate formatted as a PKCS#12 store.
    TlsPkcs12,
    /// A Kerberos keytab.
    Kerberos,
}

#[derive(Clone)]
pub enum SecretOperatorVolumeScope {
    Node,
    Pod,
    Service { name: String },
    ListenerVolume { name: String },
}

/// Reference to a listener class or listener name
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerReference {
    ListenerClass(String),
    ListenerName(String),
}

impl ListenerReference {
    /// Return the key and value for a Kubernetes object annotation
    fn to_annotation(&self) -> Result<Annotation, AnnotationError> {
        match self {
            ListenerReference::ListenerClass(class) => {
                Annotation::try_from(("listeners.stackable.tech/listener-class", class.as_str()))
            }
            ListenerReference::ListenerName(name) => {
                Annotation::try_from(("listeners.stackable.tech/listener-name", name.as_str()))
            }
        }
    }
}

// NOTE (Techassi): We might want to think about these names and how long they
// are getting.
#[derive(Debug, PartialEq, Snafu)]
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
/// let volume_source =
///     ListenerOperatorVolumeSourceBuilder::new(
///         &ListenerReference::ListenerClass("nodeport".into()),
///         &labels,
///     )
///     .unwrap()
///     .build_ephemeral()
///     .unwrap();
///
/// pod_builder
///     .add_volume(Volume {
///         name: "listener".to_string(),
///         ephemeral: Some(volume_source),
///         ..Volume::default()
///     });
///
/// // There is also a shortcut for the code above:
/// pod_builder
///     .add_listener_volume_by_listener_class("listener", "nodeport", &labels);
/// ```
#[derive(Clone, Debug)]
pub struct ListenerOperatorVolumeSourceBuilder {
    listener_reference: ListenerReference,
    labels: Labels,
}

impl ListenerOperatorVolumeSourceBuilder {
    /// Create a builder for the given listener class or listener name
    pub fn new(
        listener_reference: &ListenerReference,
        labels: &Labels,
    ) -> Result<ListenerOperatorVolumeSourceBuilder, ListenerOperatorVolumeSourceBuilderError> {
        Ok(Self {
            listener_reference: listener_reference.to_owned(),
            labels: labels.to_owned(),
        })
    }

    fn build_spec(&self) -> PersistentVolumeClaimSpec {
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
                spec: self.build_spec(),
            }),
        })
    }

    /// Build a [`PersistentVolumeClaim`] from the builder.
    pub fn build_pvc(
        &self,
        name: impl Into<String>,
    ) -> Result<PersistentVolumeClaim, ListenerOperatorVolumeSourceBuilderError> {
        let listener_reference_annotation = self
            .listener_reference
            .to_annotation()
            .context(ListenerReferenceAnnotationSnafu)?;

        Ok(PersistentVolumeClaim {
            metadata: ObjectMetaBuilder::new()
                .name(name)
                .with_annotation(listener_reference_annotation)
                .with_labels(self.labels.clone())
                .build(),
            spec: Some(self.build_spec()),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
    use std::collections::BTreeMap;

    #[test]
    fn builder() {
        let mut volume_builder = VolumeBuilder::new("name");
        volume_builder.with_config_map("configmap");
        let vol = volume_builder.build();

        assert_eq!(vol.name, "name".to_string());
        assert_eq!(
            vol.config_map.map(|cm| cm.name),
            Some("configmap".to_string())
        );

        volume_builder.with_empty_dir(Some("medium"), Some(Quantity("quantity".to_string())));
        let vol = volume_builder.build();

        assert_eq!(
            vol.empty_dir.and_then(|dir| dir.medium),
            Some("medium".to_string())
        );

        volume_builder.with_host_path("path", Some("type_"));
        let vol = volume_builder.build();

        assert_eq!(
            vol.host_path.map(|host| host.path),
            Some("path".to_string())
        );

        volume_builder.with_secret("secret", false);
        let vol = volume_builder.build();

        assert_eq!(
            vol.secret.and_then(|secret| secret.secret_name),
            Some("secret".to_string())
        );
    }

    #[test]
    fn mount_builder() {
        let mut volume_mount_builder = VolumeMountBuilder::new("name", "mount_path");
        volume_mount_builder
            .mount_propagation("mount_propagation")
            .read_only(true)
            .sub_path("sub_path")
            .sub_path_expr("sub_path_expr");

        let vm = volume_mount_builder.build();

        assert_eq!(vm.name, "name".to_string());
        assert_eq!(vm.mount_path, "mount_path".to_string());
        assert_eq!(vm.mount_propagation, Some("mount_propagation".to_string()));
        assert_eq!(vm.read_only, Some(true));
        assert_eq!(vm.sub_path, Some("sub_path".to_string()));
        assert_eq!(vm.sub_path_expr, Some("sub_path_expr".to_string()));
    }

    #[test]
    fn listener_operator_volume_source_builder() {
        let labels: Labels = Labels::try_from(BTreeMap::<String, String>::new()).unwrap();

        let builder = ListenerOperatorVolumeSourceBuilder::new(
            &ListenerReference::ListenerClass("public".into()),
            &labels,
        )
        .unwrap();

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
