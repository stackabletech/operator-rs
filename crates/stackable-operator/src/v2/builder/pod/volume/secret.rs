use k8s_openapi::{
    api::core::v1::{
        EphemeralVolumeSource, PersistentVolumeClaimSpec, PersistentVolumeClaimTemplate,
        VolumeResourceRequirements,
    },
    apimachinery::pkg::api::resource::Quantity,
};
use snafu::{ResultExt, Snafu};
use stackable_shared::time::Duration;
use tracing::warn;

use crate::{
    builder::meta::ObjectMetaBuilder,
    commons::secret_class::SecretClassVolumeProvisionParts,
    kvp::{Annotation, AnnotationError, Annotations},
};

#[derive(Debug, PartialEq, Eq, Snafu)]
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
    auto_tls_cert_lifetime: Option<Duration>,
    auto_tls_cert_domain_components_in_subject_dn: Option<bool>,
    provision_parts: SecretClassVolumeProvisionParts,
}

impl SecretOperatorVolumeSourceBuilder {
    /// Creates a builder for a secret-operator volume that uses the specified SecretClass to
    /// request the specified [`SecretClassVolumeProvisionParts`].
    ///
    /// This function forces the caller to make an explicit choice if the public parts are
    /// sufficient or if private (e.g. a certificate for the Pod) parts are needed as well.
    /// This is done to avoid accidentally requesting too much parts. For details see
    /// [this issue](https://github.com/stackabletech/issues/issues/547).
    pub fn new(
        secret_class: impl Into<String>,
        provision_parts: SecretClassVolumeProvisionParts,
    ) -> Self {
        Self {
            secret_class: secret_class.into(),
            scopes: Vec::new(),
            format: None,
            kerberos_service_names: Vec::new(),
            tls_pkcs12_password: None,
            auto_tls_cert_lifetime: None,
            auto_tls_cert_domain_components_in_subject_dn: None,
            provision_parts,
        }
    }

    pub fn with_auto_tls_cert_lifetime(&mut self, lifetime: impl Into<Duration>) -> &mut Self {
        self.auto_tls_cert_lifetime = Some(lifetime.into());
        self
    }

    pub fn with_auto_tls_cert_domain_components_in_subject_dn(
        &mut self,
        enabled: bool,
    ) -> &mut Self {
        self.auto_tls_cert_domain_components_in_subject_dn = Some(enabled);
        self
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

        #[rustfmt::skip]
        annotations
            .insert(Annotation::secret_class(&self.secret_class).context(ParseAnnotationSnafu)?)
            .insert(Annotation::secret_provision_parts(&self.provision_parts).context(ParseAnnotationSnafu)?);

        if !self.scopes.is_empty() {
            let scopes = self
                .scopes
                .iter()
                .map(crate::builder::pod::volume::SecretOperatorVolumeScope::from)
                .collect::<Vec<_>>();
            annotations.insert(Annotation::secret_scope(&scopes).context(ParseAnnotationSnafu)?);
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
            if Some(SecretFormat::TlsPkcs12) == self.format {
                annotations.insert(
                    Annotation::tls_pkcs12_password(password).context(ParseAnnotationSnafu)?,
                );
            } else {
                warn!(format.actual = ?self.format, format.expected = ?Some(SecretFormat::TlsPkcs12), "A TLS PKCS12 password was set but ignored because another format was requested");
            }
        }

        if let Some(lifetime) = &self.auto_tls_cert_lifetime {
            annotations.insert(
                Annotation::auto_tls_cert_lifetime(&lifetime.to_string())
                    .context(ParseAnnotationSnafu)?,
            );
        }

        if let Some(enabled) = self.auto_tls_cert_domain_components_in_subject_dn {
            annotations.insert(Annotation::auto_tls_cert_domain_components_in_subject_dn(
                enabled,
            ));
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

impl From<&SecretOperatorVolumeScope> for crate::builder::pod::volume::SecretOperatorVolumeScope {
    fn from(scope: &SecretOperatorVolumeScope) -> Self {
        match scope {
            SecretOperatorVolumeScope::Node => Self::Node,
            SecretOperatorVolumeScope::Pod => Self::Pod,
            SecretOperatorVolumeScope::Service { name } => Self::Service { name: name.clone() },
            SecretOperatorVolumeScope::ListenerVolume { name } => {
                Self::ListenerVolume { name: name.clone() }
            }
        }
    }
}
