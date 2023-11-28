use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::kvp::{Key, KeyValuePair, KeyValuePairError, KeyValuePairs, KeyValuePairsError};

mod value;

pub use value::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Label(KeyValuePair<LabelValue>);

impl FromStr for Label {
    type Err = KeyValuePairError<LabelValueError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kvp = KeyValuePair::from_str(s)?;
        Ok(Self(kvp))
    }
}

impl<T> TryFrom<(T, T)> for Label
where
    T: AsRef<str>,
{
    type Error = KeyValuePairError<LabelValueError>;

    fn try_from(value: (T, T)) -> Result<Self, Self::Error> {
        let kvp = KeyValuePair::try_from(value)?;
        Ok(Self(kvp))
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Label {
    /// Returns an immutable reference to the label's [`Key`].
    pub fn key(&self) -> &Key {
        self.0.key()
    }

    /// Returns an immutable reference to the label's value.
    pub fn value(&self) -> &LabelValue {
        self.0.value()
    }

    /// Consumes self and returns the inner [`KeyValuePair<LabelValue>`].
    pub fn into_inner(self) -> KeyValuePair<LabelValue> {
        self.0
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Labels(KeyValuePairs<LabelValue>);

impl TryFrom<BTreeMap<String, String>> for Labels {
    type Error = KeyValuePairError<LabelValueError>;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let kvps = KeyValuePairs::try_from(value)?;
        Ok(Self(kvps))
    }
}

impl FromIterator<KeyValuePair<LabelValue>> for Labels {
    fn from_iter<T: IntoIterator<Item = KeyValuePair<LabelValue>>>(iter: T) -> Self {
        let kvps = KeyValuePairs::from_iter(iter);
        Self(kvps)
    }
}

impl From<Labels> for BTreeMap<String, String> {
    fn from(value: Labels) -> Self {
        value.0.into()
    }
}

impl Labels {
    /// Creates a new empty list of [`Labels`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`Labels`] from `pairs`.
    pub fn new_with(pairs: BTreeSet<KeyValuePair<LabelValue>>) -> Self {
        Self(KeyValuePairs::new_with(pairs))
    }

    /// Tries to insert a new [`Label`]. It ensures there are no duplicate
    /// entries. Trying to insert duplicated data returns an error. If no such
    /// check is required, use the `insert` function instead.
    pub fn try_insert(&mut self, label: Label) -> Result<&mut Self, KeyValuePairsError> {
        self.0.try_insert(label.0)?;
        Ok(self)
    }

    /// Inserts a new [`Label`]. This function will overide any existing label
    /// already present. If this behaviour is not desired, use the `try_insert`
    /// function instead.
    pub fn insert(&mut self, kvp: KeyValuePair<LabelValue>) -> &mut Self {
        self.0.insert(kvp);
        self
    }

    /// Returns the number of labels.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns if the set of labels is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a common set of labels, which are required to identify resources
    /// that belong to a certain owner object, for example a `ZookeeperCluster`.
    /// The set contains these well-known labels:
    ///
    /// - `app.kubernetes.io/instance` and
    /// - `app.kubernetes.io/name`
    ///
    /// This function returns a result, because the parameters `app_name` and
    /// `app_instance` can contain invalid data or can exceed the maximum
    /// allowed number of characters.
    pub fn common(
        app_name: &str,
        app_instance: &str,
    ) -> Result<Self, KeyValuePairError<LabelValueError>> {
        let mut labels = Self::new();

        labels.insert(("app.kubernetes.io/instance", app_instance).try_into()?);
        labels.insert(("app.kubernetes.io/name", app_name).try_into()?);

        Ok(labels)
    }
}
