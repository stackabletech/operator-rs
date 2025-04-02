//! Utility functions and data structures the create and manage Kubernetes
//! key/value pairs, like labels and annotations.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    ops::Deref,
    str::FromStr,
};

use snafu::{ResultExt, Snafu, ensure};

use crate::iter::TryFromIterator;

mod annotation;
pub mod consts;
mod key;
mod label;
mod value;

pub use annotation::*;
pub use key::*;
pub use label::*;
pub use value::*;

/// The error type for key/value pair parsing/validating operations.
#[derive(Debug, PartialEq, Snafu)]
pub enum KeyValuePairError<E>
where
    E: std::error::Error + 'static,
{
    /// Indicates that the key failed to parse. See [`KeyError`] for more
    /// information about the error causes.
    #[snafu(display("failed to parse key {key:?} of key/value pair"))]
    InvalidKey { source: KeyError, key: String },

    /// Indicates that the value failed to parse.
    #[snafu(display("failed to parse value {value:?} of key {key:?}"))]
    InvalidValue {
        source: E,
        key: String,
        value: String,
    },
}

/// A validated Kubernetes key/value pair.
///
/// These pairs can be used as Kubernetes labels or annotations. A pair can be
/// parsed from a `(str, str)` tuple.
///
/// ### Examples
///
/// This example describes the usage of [`Label`], which is a specialized
/// [`KeyValuePair`]. The implementation makes sure that both the key (comprised
/// of optional prefix and name) and the value are validated according to the
/// Kubernetes spec linked [below](#links).
///
/// ```
/// # use stackable_operator::kvp::Label;
/// let label = Label::try_from(("stackable.tech/vendor", "Stackable")).unwrap();
/// assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
/// ```
///
/// ---
///
/// [`KeyValuePair`] is generic over the value. This allows implementors to
/// write custom validation logic for different value requirements. This
/// library provides two implementations out of the box: [`AnnotationValue`]
/// and [`LabelValue`]. Custom implementations need to implement the required
/// trait [`Value`].
///
/// ```ignore
/// use stackable_operator::kvp::{KeyValuePair, Value};
/// use serde::Serialize;
///
/// #[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
/// struct MyValue(String);
///
/// impl Value for MyValue {
///     // Implementation omitted for brevity
/// }
///
/// let kvp = KeyValuePair::<MyValue>::try_from(("key", "my_custom_value"));
/// ```
///
/// Implementing [`Value`] requires various other trait implementations like
/// [`Deref`] and [`FromStr`]. Check out the documentation for the [`Value`]
/// trait for a more detailed implementation guide.
///
/// ### Links
///
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyValuePair<T>
where
    T: Value,
{
    key: Key,
    value: T,
}

impl<K, V, T> TryFrom<(K, V)> for KeyValuePair<T>
where
    K: AsRef<str>,
    V: AsRef<str>,
    T: Value,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from(value: (K, V)) -> Result<Self, Self::Error> {
        let key = Key::from_str(value.0.as_ref()).context(InvalidKeySnafu {
            key: value.0.as_ref(),
        })?;

        let value = T::from_str(value.1.as_ref()).context(InvalidValueSnafu {
            key: key.to_string(),
            value: value.1.as_ref(),
        })?;

        Ok(Self { key, value })
    }
}

impl<T> Display for KeyValuePair<T>
where
    T: Value,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl<T> KeyValuePair<T>
where
    T: Value,
{
    /// Creates a new [`KeyValuePair`] from a validated [`Key`] and value.
    pub fn new(key: Key, value: T) -> Self {
        Self { key, value }
    }

    /// Returns an immutable reference to the pair's [`Key`].
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Returns an immutable reference to the pair's value.
    pub fn value(&self) -> &T {
        &self.value
    }
}

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyValuePairsError {
    #[snafu(display("key already exists"))]
    KeyAlreadyExists,
}

/// A validated set/list of Kubernetes key/value pairs.
///
/// It implements various traits which allows conversion from and to different
/// data types. Traits to construct [`KeyValuePairs`] from other data types are:
///
/// - `TryFrom<&BTreeMap<String, String>>`
/// - `TryFrom<BTreeMap<String, String>>`
/// - `FromIterator<KeyValuePair<T>>`
/// - `TryFrom<[(K, V); N]>`
///
/// Traits to convert [`KeyValuePairs`] into a different data type are:
///
/// - `From<KeyValuePairs<T>> for BTreeMap<String, String>`
///
/// See [`Labels`] and [`Annotations`] on how these traits can be used.
///
/// # Note
///
/// A [`BTreeSet`] is used as the inner collection to preserve order of items
/// which ultimately prevent unncessary reconciliations due to changes
/// in item order.
#[derive(Clone, Debug, Default)]
pub struct KeyValuePairs<T: Value>(BTreeMap<Key, T>);

impl<K, V, T> TryFrom<BTreeMap<K, V>> for KeyValuePairs<T>
where
    K: AsRef<str>,
    V: AsRef<str>,
    T: Value,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from(map: BTreeMap<K, V>) -> Result<Self, Self::Error> {
        Self::try_from_iter(map)
    }
}

impl<K, V, T> TryFrom<&BTreeMap<K, V>> for KeyValuePairs<T>
where
    K: AsRef<str>,
    V: AsRef<str>,
    T: Value,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from(map: &BTreeMap<K, V>) -> Result<Self, Self::Error> {
        Self::try_from_iter(map)
    }
}

impl<const N: usize, K, V, T> TryFrom<[(K, V); N]> for KeyValuePairs<T>
where
    K: AsRef<str>,
    V: AsRef<str>,
    T: Value + std::default::Default,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from(array: [(K, V); N]) -> Result<Self, Self::Error> {
        Self::try_from_iter(array)
    }
}

impl<T> FromIterator<KeyValuePair<T>> for KeyValuePairs<T>
where
    T: Value,
{
    fn from_iter<I: IntoIterator<Item = KeyValuePair<T>>>(iter: I) -> Self {
        Self(iter.into_iter().map(|kvp| (kvp.key, kvp.value)).collect())
    }
}

impl<K, V, T> TryFromIterator<(K, V)> for KeyValuePairs<T>
where
    K: AsRef<str>,
    V: AsRef<str>,
    T: Value,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Result<Self, Self::Error> {
        let pairs = iter
            .into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<BTreeSet<_>, KeyValuePairError<T::Error>>>()?;

        Ok(Self::from_iter(pairs))
    }
}

impl<T> From<KeyValuePairs<T>> for BTreeMap<String, String>
where
    T: Value,
{
    fn from(value: KeyValuePairs<T>) -> Self {
        value
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

impl<T> Deref for KeyValuePairs<T>
where
    T: Value,
{
    type Target = BTreeMap<Key, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> KeyValuePairs<T>
where
    T: Value + std::default::Default,
{
    /// Creates a new empty list of [`KeyValuePair`]s.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`KeyValuePair`]s from `pairs`.
    pub fn new_with(pairs: BTreeSet<KeyValuePair<T>>) -> Self {
        Self::from_iter(pairs)
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// Inserts a new [`KeyValuePair`] into the list of pairs.
    ///
    /// This function overwrites any existing key/value pair. To avoid
    /// overwriting existing pairs, either use [`KeyValuePairs::contains`] or
    /// [`KeyValuePairs::contains_key`] before inserting or try to insert
    /// fallible via [`KeyValuePairs::try_insert`].
    pub fn insert(&mut self, kvp: KeyValuePair<T>) -> &mut Self {
        self.0.insert(kvp.key, kvp.value);
        self
    }

    /// Tries to insert a new [`KeyValuePair`] into the list of pairs.
    ///
    /// If the list already had this key present, nothing is updated, and an
    /// error is returned.
    pub fn try_insert(&mut self, kvp: KeyValuePair<T>) -> Result<(), KeyValuePairsError> {
        ensure!(!self.0.contains_key(&kvp.key), KeyAlreadyExistsSnafu);
        self.insert(kvp);
        Ok(())
    }

    /// Returns if the list contains a specific [`KeyValuePair`].
    pub fn contains(&self, kvp: impl TryInto<KeyValuePair<T>>) -> bool {
        let Ok(kvp) = kvp.try_into() else {
            return false;
        };
        let Some(value) = self.get(&kvp.key) else {
            return false;
        };
        value == &kvp.value
    }

    /// Returns if the list contains a key/value pair with a specific [`Key`].
    pub fn contains_key(&self, key: impl TryInto<Key>) -> bool {
        let Ok(key) = key.try_into() else {
            return false;
        };

        self.0.contains_key(&key)
    }

    /// Returns an [`Iterator`] over [`KeyValuePairs`] yielding a reference to every [`KeyValuePair`] contained within.
    pub fn iter(&self) -> impl Iterator<Item = KeyValuePair<T>> + '_ {
        self.0.iter().map(|(k, v)| KeyValuePair {
            key: k.clone(),
            value: v.clone(),
        })
    }
}

impl<T> IntoIterator for KeyValuePairs<T>
where
    T: Value,
{
    type IntoIter =
        std::iter::Map<std::collections::btree_map::IntoIter<Key, T>, fn((Key, T)) -> Self::Item>;
    type Item = KeyValuePair<T>;

    /// Returns a consuming [`Iterator`] over [`KeyValuePairs`] moving every [`KeyValuePair`] out.
    /// The [`KeyValuePairs`] cannot be used again after calling this.
    fn into_iter(self) -> Self::IntoIter {
        self.0
            .into_iter()
            .map(|(key, value)| KeyValuePair { key, value })
    }
}

/// A recommended set of labels to set on objects created by Stackable
/// operators or management tools.
#[derive(Debug, Clone, Copy)]
pub struct ObjectLabels<'a, T> {
    /// Reference to the k8s object owning the created resource, such as
    /// `HdfsCluster` or `TrinoCluster`.
    pub owner: &'a T,

    /// The name of the app being managed, such as `zookeeper`.
    pub app_name: &'a str,

    /// The version of the app being managed (not of the operator).
    ///
    /// If setting this label on a Stackable product then please use
    /// [`ResolvedProductImage::app_version_label`][avl].
    ///
    /// This version should include the Stackable version, such as
    /// `3.0.0-stackable23.11`. If the Stackable version is not known, then
    /// the product version should be used together with a suffix (if possible).
    /// If a custom product image is provided by the user (in which case only
    /// the product version is known), then the format `3.0.0-<tag-of-custom-image>`
    /// should be used.
    ///
    /// However, this is pure documentation and should not be parsed.
    ///
    /// [avl]: crate::commons::product_image_selection::ResolvedProductImage::app_version_label
    pub app_version: &'a str,

    /// The DNS-style name of the operator managing the object (such as `zookeeper.stackable.tech`)
    pub operator_name: &'a str,

    /// The name of the controller inside of the operator managing the object (such as `zookeepercluster`)
    pub controller_name: &'a str,

    /// The role that this object belongs to
    pub role: &'a str,

    /// The role group that this object belongs to
    pub role_group: &'a str,
}

#[cfg(test)]
mod test {
    use snafu::Report;

    use super::*;

    #[test]
    fn try_from_tuple() {
        let label = Label::try_from(("stackable.tech/vendor", "Stackable")).unwrap();

        assert_eq!(
            label.key(),
            &Key::from_str("stackable.tech/vendor").unwrap()
        );
        assert_eq!(label.value(), &LabelValue::from_str("Stackable").unwrap());

        assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
    }

    #[test]
    fn labels_from_array() {
        let labels = Labels::try_from([
            ("stackable.tech/managed-by", "stackablectl"),
            ("stackable.tech/vendor", "Stackable"),
        ])
        .unwrap();

        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn labels_from_iter() {
        let labels = Labels::from_iter([
            KeyValuePair::try_from(("stackable.tech/managed-by", "stackablectl")).unwrap(),
            KeyValuePair::try_from(("stackable.tech/vendor", "Stackable")).unwrap(),
        ]);

        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn labels_try_from_map() {
        let map = BTreeMap::from([
            ("stackable.tech/managed-by", "stackablectl"),
            ("stackable.tech/vendor", "Stackable"),
        ]);

        let labels = Labels::try_from(map).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn labels_into_map() {
        let labels = Labels::try_from([
            ("stackable.tech/managed-by", "stackablectl"),
            ("stackable.tech/vendor", "Stackable"),
        ])
        .unwrap();

        let map: BTreeMap<String, String> = labels.into();
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn contains() {
        let labels = Labels::common("test", "test-01").unwrap();

        assert!(labels.contains(("app.kubernetes.io/name", "test")));
        assert!(labels.contains_key("app.kubernetes.io/instance"))
    }

    #[test]
    fn try_from_iter() {
        let map = BTreeMap::from([
            ("stackable.tech/managed-by", "stackablectl"),
            ("stackable.tech/vendor", "Stackable"),
        ]);

        let labels = Labels::try_from_iter(map).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn key_error() {
        let err = Label::try_from(("stäckable.tech/vendor", "Stackable")).unwrap_err();
        let report = Report::from_error(err);
        println!("{report}")
    }

    #[test]
    fn value_error() {
        let err = Label::try_from(("stackable.tech/vendor", "Stäckable")).unwrap_err();
        let report = Report::from_error(err);
        println!("{report}")
    }

    #[test]
    fn merge() {
        let mut merged_labels =
            Labels::try_from_iter([("a", "b"), ("b", "a"), ("c", "c")]).unwrap();
        merged_labels.extend(Labels::try_from_iter([("a", "a"), ("b", "b"), ("d", "d")]).unwrap());
        assert_eq!(
            BTreeMap::from(merged_labels),
            BTreeMap::from(
                Labels::try_from_iter([("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")]).unwrap()
            )
        )
    }
}
