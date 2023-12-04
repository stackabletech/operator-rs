use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::{
    builder::SecretOperatorVolumeScope,
    kvp::{Key, KeyValuePair, KeyValuePairError, KeyValuePairs, KeyValuePairsError},
};

mod value;

pub use value::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Annotation(KeyValuePair<AnnotationValue>);

impl FromStr for Annotation {
    type Err = KeyValuePairError<AnnotationValueError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kvp = KeyValuePair::from_str(s)?;
        Ok(Self(kvp))
    }
}

impl<T> TryFrom<(T, T)> for Annotation
where
    T: AsRef<str>,
{
    type Error = KeyValuePairError<AnnotationValueError>;

    fn try_from(value: (T, T)) -> Result<Self, Self::Error> {
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

    pub fn secret_class(
        secret_class: &str,
    ) -> Result<Self, KeyValuePairError<AnnotationValueError>> {
        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/class", secret_class))?;
        Ok(Self(kvp))
    }

    pub fn secret_scope(
        scopes: impl AsRef<[SecretOperatorVolumeScope]>,
    ) -> Result<Self, KeyValuePairError<AnnotationValueError>> {
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
                    value.push_str(&name);
                }
            }
        }

        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/scope", value))?;
        Ok(Self(kvp))
    }

    pub fn secret_format(format: &str) -> Result<Self, KeyValuePairError<AnnotationValueError>> {
        let kvp = KeyValuePair::try_from(("secrets.stackable.tech/format", format))?;
        Ok(Self(kvp))
    }

    pub fn kerberos_service_names(
        names: impl AsRef<[String]>,
    ) -> Result<Self, KeyValuePairError<AnnotationValueError>> {
        let names = names.as_ref().join(",");
        let kvp = KeyValuePair::try_from(("secret", names))?;
        Ok(Self(kvp))
    }

    pub fn tls_pkcs12_password(
        password: &str,
    ) -> Result<Self, KeyValuePairError<AnnotationValueError>> {
        let kvp = KeyValuePair::try_from((
            "secrets.stackable.tech/format.compatibility.tls-pkcs12.password",
            password,
        ))?;
        Ok(Self(kvp))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Annotations(KeyValuePairs<AnnotationValue>);

impl TryFrom<BTreeMap<String, String>> for Annotations {
    type Error = KeyValuePairError<AnnotationValueError>;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
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

    /// Tries to insert a new [`Annotation`]. It ensures there are no duplicate
    /// entries. Trying to insert duplicated data returns an error. If no such
    /// check is required, use the `insert` function instead.
    pub fn try_insert(
        &mut self,
        annotation: Annotation,
    ) -> Result<&mut Self, KeyValuePairsError<AnnotationValueError>> {
        self.0.try_insert(annotation.0)?;
        Ok(self)
    }

    /// Inserts a new [`Annotation`]. This function will overide any existing
    /// annotation already present. If this behaviour is not desired, use the
    /// `try_insert` function instead.
    pub fn insert(&mut self, annotation: Annotation) -> &mut Self {
        self.0.insert(annotation.0);
        self
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0)
    }

    /// Returns the number of annotations.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns if the set of annotations is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
