use crate::{
    commons::secret_class::SecretClassVolumeProvisionParts,
    kvp::Annotation,
    v2::{builder::pod::volume::SecretOperatorVolumeScope, types::kubernetes::SecretClassName},
};

/// Constructs a `secrets.stackable.tech/provision-parts` annotation.
pub fn secret_provision_parts(provision_parts: &SecretClassVolumeProvisionParts) -> Annotation {
    Annotation::secret_provision_parts(provision_parts)
        .expect("The statically defined annotation key, combined with any SecretClassVolumeProvisionParts, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/class` annotation.
pub fn secret_class(secret_class: &SecretClassName) -> Annotation {
    Annotation::secret_class(secret_class.as_ref())
        .expect("The statically defined annotation key, combined with any SecretClass name, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/scope` annotation.
pub fn secret_scope(scopes: impl AsRef<[SecretOperatorVolumeScope]>) -> Annotation {
    let scopes = scopes
        .as_ref()
        .iter()
        .map(crate::builder::pod::volume::SecretOperatorVolumeScope::from)
        .collect::<Vec<_>>();
    Annotation::secret_scope(scopes)
        .expect("The statically defined annotation key, combined with any SecretOperatorVolumeScope name, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/format` annotation.
pub fn secret_format(format: &str) -> Annotation {
    Annotation::secret_format(format)
        .expect("The statically defined annotation key, combined with any UTF-8 string, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/kerberos.service.names` annotation.
pub fn kerberos_service_names(names: impl AsRef<[String]>) -> Annotation {
    Annotation::kerberos_service_names(names)
        .expect("The statically defined annotation key, combined with any UTF-8 string, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/format.compatibility.tls-pkcs12.password`
/// annotation.
pub fn tls_pkcs12_password(password: &str) -> Annotation {
    Annotation::tls_pkcs12_password(password)
        .expect("The statically defined annotation key, combined with any UTF-8 string, produces a valid annotation.")
}

/// Constructs a `secrets.stackable.tech/backend.autotls.cert.lifetime` annotation.
pub fn auto_tls_cert_lifetime(lifetime: &str) -> Annotation {
    Annotation::auto_tls_cert_lifetime(lifetime)
        .expect("The statically defined annotation key, combined with any UTF-8 string, produces a valid annotation.")
}

/// Constructs a `autoscaling.stackable.tech/retry` annotation.
pub fn autoscaling_retry(retry: bool) -> Annotation {
    Annotation::autoscaling_retry(retry)
}

/// Constructs a `secrets.stackable.tech/backend.autotls.cert.domain-components-in-subject-dn` annotation.
pub fn auto_tls_cert_domain_components_in_subject_dn(enabled: bool) -> Annotation {
    Annotation::auto_tls_cert_domain_components_in_subject_dn(enabled)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::v2::types::kubernetes::{ServiceName, VolumeName};

    #[test]
    fn static_annotation_keys_are_valid() {
        secret_provision_parts(&SecretClassVolumeProvisionParts::PublicPrivate);
        secret_class(&SecretClassName::from_str_unsafe("my-secret-class"));
        secret_scope([
            SecretOperatorVolumeScope::Node,
            SecretOperatorVolumeScope::Pod,
            SecretOperatorVolumeScope::Service {
                name: ServiceName::from_str_unsafe("my-service"),
            },
            SecretOperatorVolumeScope::ListenerVolume {
                name: VolumeName::from_str_unsafe("my-volume"),
            },
        ]);
        secret_format("pem");
        kerberos_service_names(["my-service-1".to_owned(), "my-service-2".to_owned()]);
        tls_pkcs12_password("changeit");
        auto_tls_cert_lifetime("1d");
        autoscaling_retry(true);
        auto_tls_cert_domain_components_in_subject_dn(true);
    }
}
