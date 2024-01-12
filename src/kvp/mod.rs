//! Utility functions and data structures the create and manage Kubernetes
//! key/value pairs, like labels and annotations.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    ops::Deref,
    str::FromStr,
};

use snafu::{ensure, ResultExt, Snafu};

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
    #[snafu(display("failed to parse key of key/value pair"))]
    InvalidKey { source: KeyError },

    /// Indicates that the value failed to parse.
    #[snafu(display("failed to parse value of key/value pair"))]
    InvalidValue { source: E },
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
        let key = Key::from_str(value.0.as_ref()).context(InvalidKeySnafu)?;
        let value = T::from_str(value.1.as_ref()).context(InvalidValueSnafu)?;

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

#[derive(Debug, Snafu)]
pub enum KeyValuePairsError {
    #[snafu(display("key/value pair already exists"))]
    PairAlreadyExists,
}

/// A validated set/list of Kubernetes key/value pairs.
#[derive(Clone, Debug, Default)]
pub struct KeyValuePairs<T: Value>(BTreeSet<KeyValuePair<T>>);

impl<T> TryFrom<BTreeMap<String, String>> for KeyValuePairs<T>
where
    T: Value,
{
    type Error = KeyValuePairError<T::Error>;

    fn try_from(map: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let pairs = map
            .into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<BTreeSet<_>, KeyValuePairError<T::Error>>>()?;

        Ok(Self(pairs))
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
        let mut pairs = KeyValuePairs::new();

        for item in array {
            pairs.insert(KeyValuePair::try_from(item)?);
        }

        Ok(pairs)
    }
}

impl<T> FromIterator<KeyValuePair<T>> for KeyValuePairs<T>
where
    T: Value,
{
    fn from_iter<I: IntoIterator<Item = KeyValuePair<T>>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<T> From<KeyValuePairs<T>> for BTreeMap<String, String>
where
    T: Value,
{
    fn from(value: KeyValuePairs<T>) -> Self {
        value
            .iter()
            .map(|pair| (pair.key().to_string(), pair.value().to_string()))
            .collect()
    }
}

impl<T> Deref for KeyValuePairs<T>
where
    T: Value,
{
    type Target = BTreeSet<KeyValuePair<T>>;

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
        Self(pairs)
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
        self.0.insert(kvp);
        self
    }

    /// Tries to insert a new [`KeyValuePair`] into the list of pairs.
    ///
    /// If the list already had this pair present, nothing is updated, and an
    /// error is returned.
    pub fn try_insert(&mut self, kvp: KeyValuePair<T>) -> Result<(), KeyValuePairsError> {
        ensure!(!self.0.contains(&kvp), PairAlreadyExistsSnafu);
        self.insert(kvp);
        Ok(())
    }

    /// Returns if the list contains a specific [`KeyValuePair`].
    pub fn contains(&self, kvp: impl TryInto<KeyValuePair<T>>) -> bool {
        let Ok(kvp) = kvp.try_into() else {return false};
        self.0.contains(&kvp)
    }

    /// Returns if the list contains a key/value pair with a specific [`Key`].
    pub fn contains_key(&self, key: impl TryInto<Key>) -> bool {
        let Ok(key) = key.try_into() else {return false};

        for kvp in &self.0 {
            if kvp.key == key {
                return true;
            }
        }

        false
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
            ("stackable.tech/vendor".to_string(), "Stackable".to_string()),
            (
                "stackable.tech/managed-by".to_string(),
                "stackablectl".to_string(),
            ),
        ]);

        let labels = Labels::try_from(map).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn labels_into_map() {
        let pairs = BTreeSet::from([
            KeyValuePair::try_from(("stackable.tech/managed-by", "stackablectl")).unwrap(),
            KeyValuePair::try_from(("stackable.tech/vendor", "Stackable")).unwrap(),
        ]);

        let labels = Labels::new_with(pairs);
        let map: BTreeMap<String, String> = labels.into();

        assert_eq!(map.len(), 2);
    }

    #[test]
    fn contains() {
        let labels = Labels::common("test", "test-01").unwrap();

        assert!(labels.contains(("app.kubernetes.io/name", "test")));
        assert!(labels.contains_key("app.kubernetes.io/instance"))
    }
}
