use std::collections::BTreeMap;

use crate::types::{Annotation, KeyValuePairExt, KeyValuePairParseError, Label};

pub type AnnotationListBuilder = KeyValuePairBuilder<Annotation>;
pub type LabelListBuilder = KeyValuePairBuilder<Label>;

pub struct KeyValuePairBuilder<P: KeyValuePairExt> {
    prefix: Option<String>,
    kvps: Vec<P>,
}

impl<P: KeyValuePairExt> KeyValuePairBuilder<P> {
    /// Creates a new key/value pair list builder with items of type `P`. The
    /// optional `prefix` will be attached to each added pair via the
    /// [`KeyValuePairBuilder::add`] method. This builder makes it easier to
    /// build a list of key/value pairs which all share a **common** key prefix,
    /// like `app.kubernetes.io/<name>=<value>`.
    pub fn new<T>(prefix: Option<T>) -> Self
    where
        T: Into<String>,
    {
        Self {
            prefix: prefix.map(Into::into),
            kvps: Vec::new(),
        }
    }

    /// Tries to add a new key/value pair to this builder. This method ensures
    /// that the optional key prefix is valid and in addition also validates
    /// the key name. Both values must not exceed the maximum length of 253
    /// characters for the prefix and 63 characters for the name. Also, both
    /// need to only contain allowed characters.
    pub fn add<T>(&mut self, name: T, value: T) -> Result<&mut Self, KeyValuePairParseError>
    where
        T: Into<String>,
    {
        let kvp = P::new(self.prefix.clone(), name.into(), value.into())?;
        self.kvps.push(kvp);
        Ok(self)
    }

    /// Builds a [`BTreeMap<String, P>`] out of the added pairs. This mthods is
    /// useful when we need a map of items with the underlying type `P` still
    /// preserved. The Kubernetes API uses [`BTreeMap<String, String>`] to
    /// handle lists of labels and annotations. The build such a map, use the
    /// `build_raw` method instead.
    pub fn build(self) -> BTreeMap<String, P> {
        self.kvps.into_iter().map(|kvp| (kvp.key(), kvp)).collect()
    }

    /// Builds a [`BTreeMap<String, String>`] out of the added pairs. This
    /// method is useful when the returned value is directly passed to
    /// the Kubernetes API or data structures.
    pub fn build_raw(self) -> BTreeMap<String, String> {
        self.kvps
            .into_iter()
            .map(|kvp| (kvp.key(), kvp.value().clone()))
            .collect()
    }
}

pub trait KeyValuePairMapExt {
    fn into_raw(self) -> BTreeMap<String, String>;
}

impl<P: KeyValuePairExt> KeyValuePairMapExt for BTreeMap<String, P> {
    fn into_raw(self) -> BTreeMap<String, String> {
        self.into_iter()
            .map(|(name, kvp)| (name, kvp.value().clone()))
            .collect()
    }
}
