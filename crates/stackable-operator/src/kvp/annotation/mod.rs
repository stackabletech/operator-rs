//! This module provides various types and functions to construct valid Kubernetes
//! annotations. Annotations are key/value pairs, where the key must meet certain
//! requirementens regarding length and character set. The value can contain
//! **any** valid UTF-8 data.
//!
//! Additionally, the [`Annotation`] struct provides various helper functions to
//! construct commonly used annotations across the Stackable Data Platform, like
//! the secret scope or class.
//!
//! See <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
//! for more information on Kubernetes annotations.
use std::convert::Infallible;

use crate::kvp::{KeyValuePair, KeyValuePairError, KeyValuePairs};

mod value;

pub use value::*;

/// A type alias for errors returned when construction or manipulation of a set
/// of annotations fails.
pub type AnnotationError = KeyValuePairError<Infallible>;

/// A specialized implementation of a key/value pair representing Kubernetes
/// annotations.
///
/// The validation of the annotation value can **never** fail, as [`str`] is
/// guaranteed  to only contain valid UTF-8 data - which is the only
/// requirement for a valid Kubernetes annotation value.
///
/// See <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
/// for more information on Kubernetes annotations.
pub type Annotation = KeyValuePair<AnnotationValue>;

/// A validated set/list of Kubernetes annotations.
///
/// It provides selected associated functions to manipulate the set of
/// annotations, like inserting or extending.
///
/// ## Examples
///
/// ### Converting a BTreeMap into a list of labels
///
/// ```
/// # use std::collections::BTreeMap;
/// # use stackable_operator::iter::TryFromIterator;
/// # use stackable_operator::kvp::Annotations;
/// let map = BTreeMap::from([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stäckable"),
/// ]);
///
/// let labels = Annotations::try_from_iter(map).unwrap();
/// ```
///
/// ### Creating a list of labels from an array
///
/// ```
/// # use stackable_operator::iter::TryFromIterator;
/// # use stackable_operator::kvp::Annotations;
/// let labels = Annotations::try_from_iter([
///     ("stackable.tech/managed-by", "stackablectl"),
///     ("stackable.tech/vendor", "Stäckable"),
/// ]).unwrap();
/// ```
pub type Annotations = KeyValuePairs<AnnotationValue>;

/// Well-known annotations used by other tools or standard conventions.
pub mod well_known {
    /// Annotations applicable to Stackable Secret Operator volumes
    pub mod secret_volume {
        use crate::{
            builder::pod::volume::SecretOperatorVolumeScope,
            kvp::{Annotation, AnnotationError},
        };

        /// Constructs a `secrets.stackable.tech/class` annotation.
        pub fn secret_class(secret_class: &str) -> Result<Annotation, AnnotationError> {
            Annotation::try_from(("secrets.stackable.tech/class", secret_class))
        }

        /// Constructs a `secrets.stackable.tech/scope` annotation.
        pub fn secret_scope(
            scopes: &[SecretOperatorVolumeScope],
        ) -> Result<Annotation, AnnotationError> {
            let mut value = String::new();

            for scope in scopes {
                if !value.is_empty() {
                    value.push(',');
                }

                match scope {
                    SecretOperatorVolumeScope::Node => value.push_str("node"),
                    SecretOperatorVolumeScope::Pod => value.push_str("pod"),
                    SecretOperatorVolumeScope::Service { name } => {
                        value.push_str("service=");
                        value.push_str(name);
                    }
                    SecretOperatorVolumeScope::ListenerVolume { name } => {
                        value.push_str("listener-volume=");
                        value.push_str(name);
                    }
                }
            }

            Annotation::try_from(("secrets.stackable.tech/scope", value.as_str()))
        }

        /// Constructs a `secrets.stackable.tech/format` annotation.
        pub fn secret_format(format: &str) -> Result<Annotation, AnnotationError> {
            Annotation::try_from(("secrets.stackable.tech/format", format))
        }

        /// Constructs a `secrets.stackable.tech/kerberos.service.names` annotation.
        pub fn kerberos_service_names(names: &[String]) -> Result<Annotation, AnnotationError> {
            let names = names.join(",");
            Annotation::try_from((
                "secrets.stackable.tech/kerberos.service.names",
                names.as_str(),
            ))
        }

        /// Constructs a `secrets.stackable.tech/format.compatibility.tls-pkcs12.password`
        /// annotation.
        pub fn tls_pkcs12_password(password: &str) -> Result<Annotation, AnnotationError> {
            Annotation::try_from((
                "secrets.stackable.tech/format.compatibility.tls-pkcs12.password",
                password,
            ))
        }
    }
}
