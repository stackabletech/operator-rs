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
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fmt::Display,
};

use delegate::delegate;

use crate::{
    builder::SecretOperatorVolumeScope,
    kvp::{Key, KeyValuePair, KeyValuePairError, KeyValuePairs, KeyValuePairsError},
};

mod value;

pub use value::*;

pub type AnnotationsError = KeyValuePairsError;

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
#[derive(Debug)]
pub struct Annotation(KeyValuePair<AnnotationValue>);

impl<T, K> TryFrom<(T, K)> for Annotation
where
    T: AsRef<str>,
    K: AsRef<str>,
{
    type Error = AnnotationError;

    fn try_from(value: (T, K)) -> Result<Self, Self::Error> {
        let kvp = KeyValuePair::try_from(value)?;
        Ok(Self(kvp))
    }
}

impl Display for Annotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Annotation {
    /// Returns an immutable reference to the annotation's [`Key`].
    pub fn key(&self) -> &Key {
        self.0.key()
    }

    /// Returns an immutable reference to the annotation's value.
    pub fn value(&self) -> &AnnotationValue {
        self.0.value()
    }

    /// Consumes self and returns the inner [`KeyValuePair<AnnotationValue>`].
    pub fn into_inner(self) -> KeyValuePair<AnnotationValue> {
        self.0
    }

    /// Constructs a `secrets.stackable.tech/class` annotation.
    pub fn secret_class(secret_class: &str) -> Result<Self, AnnotationError> {
        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/class", secret_class))?;
        Ok(Self(kvp))
    }

    /// Constructs a `secrets.stackable.tech/scope` annotation.
    pub fn secret_scope(
        scopes: impl AsRef<[SecretOperatorVolumeScope]>,
    ) -> Result<Self, AnnotationError> {
        let mut value = String::new();

        for scope in scopes.as_ref() {
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
            }
        }

        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/scope", value))?;
        Ok(Self(kvp))
    }

    /// Constructs a `secrets.stackable.tech/format` annotation.
    pub fn secret_format(format: &str) -> Result<Self, AnnotationError> {
        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/format", format))?;
        Ok(Self(kvp))
    }

    /// Constructs a `secrets.stackable.tech/kerberos.service.names` annotation.
    pub fn kerberos_service_names(names: impl AsRef<[String]>) -> Result<Self, AnnotationError> {
        let names = names.as_ref().join(",");
        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/kerberos.service.names", names))?;
        Ok(Self(kvp))
    }

    /// Constructs a `secrets.stackable.tech/format.compatibility.tls-pkcs12.password`
    /// annotation.
    pub fn tls_pkcs12_password(password: &str) -> Result<Self, AnnotationError> {
        let kvp = KeyValuePair::try_from((
            "secrets.stackable.tech/format.compatibility.tls-pkcs12.password",
            password,
        ))?;
        Ok(Self(kvp))
    }
}

/// A validated set/list of Kubernetes annotations.
///
/// It provides selected associated functions to manipulate the set of
/// annotations, like inserting or extending.
#[derive(Clone, Debug, Default)]
pub struct Annotations(KeyValuePairs<AnnotationValue>);

impl TryFrom<BTreeMap<String, String>> for Annotations {
    type Error = AnnotationError;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let kvps = KeyValuePairs::try_from(value)?;
        Ok(Self(kvps))
    }
}

impl<const N: usize, T, K> TryFrom<[(T, K); N]> for Annotations
where
    T: AsRef<str>,
    K: AsRef<str>,
{
    type Error = AnnotationError;

    fn try_from(value: [(T, K); N]) -> Result<Self, Self::Error> {
        let kvps = KeyValuePairs::try_from(value)?;
        Ok(Self(kvps))
    }
}

impl FromIterator<KeyValuePair<AnnotationValue>> for Annotations {
    fn from_iter<T: IntoIterator<Item = KeyValuePair<AnnotationValue>>>(iter: T) -> Self {
        let kvps = KeyValuePairs::from_iter(iter);
        Self(kvps)
    }
}

impl From<Annotations> for BTreeMap<String, String> {
    fn from(value: Annotations) -> Self {
        value.0.into()
    }
}

impl Annotations {
    /// Creates a new empty list of [`Annotations`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`Annotations`] from `pairs`.
    pub fn new_with(pairs: BTreeSet<KeyValuePair<AnnotationValue>>) -> Self {
        Self(KeyValuePairs::new_with(pairs))
    }

    /// Tries to insert a new annotation by first parsing `annotation` as an
    /// [`Annotation`] and then inserting it into the list. This function will
    /// overide any existing annotation already present.
    pub fn parse_insert(
        &mut self,
        annotation: impl TryInto<Annotation, Error = AnnotationError>,
    ) -> Result<(), AnnotationError> {
        self.0.insert(annotation.try_into()?.0);
        Ok(())
    }

    /// Inserts a new [`Annotation`]. This function will overide any existing
    /// annotation already present.
    pub fn insert(&mut self, annotation: Annotation) -> &mut Self {
        self.0.insert(annotation.0);
        self
    }

    // This forwards / delegates associated functions to the inner field. In
    // this case self.0 which is of type KeyValuePairs<T>. So calling
    // Annotations::len() will be delegated to KeyValuePair<T>::len() without
    // the need to write boilerplate code.
    delegate! {
        to self.0 {
            /// Tries to insert a new [`Annotation`]. It ensures there are no duplicate
            /// entries. Trying to insert duplicated data returns an error. If no such
            /// check is required, use the `insert` function instead.
            pub fn try_insert(&mut self, #[newtype] annotation: Annotation) -> Result<(), AnnotationsError>;

            /// Extends `self` with `other`.
            pub fn extend(&mut self, #[newtype] other: Self);

            /// Returns the number of labels.
            pub fn len(&self) -> usize;

            /// Returns if the set of labels is empty.
            pub fn is_empty(&self) -> bool;

            /// Returns if the set of annotations contains the provided
            /// `annotation`. Failure to parse/validate the [`KeyValuePair`]
            /// will return `false`.
            pub fn contains(&self, annotation: impl TryInto<KeyValuePair<AnnotationValue>>) -> bool;

            /// Returns if the set of annotations contains a label with the
            /// provided `key`. Failure to parse/validate the [`Key`] will
            /// return `false`.
            pub fn contains_key(&self, key: impl TryInto<Key>) -> bool;
        }
    }
}
