//! Utility functions and data structures the create and manage Kubernetes
//! key/value pairs, like labels and annotations.
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    str::FromStr,
};

use snafu::{ResultExt, Snafu};

use crate::iter::TryFromIterator;

pub mod annotation;
pub mod consts;
pub mod label;

mod key;
mod value;

pub use annotation::{Annotation, AnnotationError, AnnotationValue, Annotations};
pub use key::*;
pub use label::{Label, LabelError, LabelSelectorExt, LabelValue, Labels, SelectorError};
pub use value::*;

#[cfg(doc)]
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
#[cfg(doc)]
use std::ops::Deref;

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
    #[snafu(display("failed to parse value {value:?} for key {key:?}", key = key.to_string()))]
    InvalidValue { source: E, key: Key, value: String },
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
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyValuePair<V>
where
    V: Value,
{
    pub key: Key,
    pub value: V,
}

impl<V> TryFrom<(&str, &str)> for KeyValuePair<V>
where
    V: Value,
{
    type Error = KeyValuePairError<V::Error>;

    fn try_from((key, value): (&str, &str)) -> Result<Self, Self::Error> {
        let key = Key::from_str(key).context(InvalidKeySnafu { key })?;
        let value = V::from_str(value).context(InvalidValueSnafu { key: &key, value })?;
        Ok(Self { key, value })
    }
}

impl<V: Value> From<KeyValuePair<V>> for (Key, V) {
    fn from(KeyValuePair { key, value }: KeyValuePair<V>) -> Self {
        (key, value)
    }
}

impl<V: Value> Display for KeyValuePair<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl<V: Value + Debug> Debug for KeyValuePair<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {:?}", self.key, self.value)
    }
}

/// A validated set/list of Kubernetes key/value pairs.
///
/// See [`Annotations`] and [`Labels`] for actual instantiations.
///
/// See [`KeyValuePairsExt`] for kvp-specific convenience helpers.
pub type KeyValuePairs<V> = BTreeMap<Key, V>;

impl<V: Value> Extend<KeyValuePair<V>> for KeyValuePairs<V> {
    fn extend<T: IntoIterator<Item = KeyValuePair<V>>>(&mut self, iter: T) {
        self.extend(iter.into_iter().map(<(Key, V)>::from));
    }
}

impl<V: Value> FromIterator<KeyValuePair<V>> for KeyValuePairs<V> {
    fn from_iter<T: IntoIterator<Item = KeyValuePair<V>>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter().map(<(Key, V)>::from))
    }
}

/// Helpers for [`KeyValuePairs`].
pub trait KeyValuePairsExt {
    /// Clones `self` into a type without validation types, ready for use in [`ObjectMeta::annotations`]/[`ObjectMeta::labels`].
    fn to_unvalidated(&self) -> BTreeMap<String, String>;

    /// Returns whether the list contains a key/value pair with a specific [`Key`].
    ///
    /// Returns `false` if `key` cannot be parsed as a valid [`Key`].
    // TODO: Does anyone actually use this API?
    fn contains_str_key(&self, key: &str) -> bool;
}
impl<V: Value> KeyValuePairsExt for KeyValuePairs<V> {
    fn to_unvalidated(&self) -> BTreeMap<String, String> {
        self.iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn contains_str_key(&self, key: &str) -> bool {
        // We could avoid this clone by providing an UnvalidatedKeyRef and ensure that Key: Borrow<UnvalidatedKeyRef>
        let Ok(key) = key.parse::<Key>() else {
            // If the key cannot be parsed then it cannot, by definition, possibly exist in the map
            return false;
        };
        self.contains_key(&key)
    }
}

impl<'a, V: Value> TryFromIterator<(&'a str, &'a str)> for KeyValuePairs<V> {
    type Error = KeyValuePairError<V::Error>;

    fn try_from_iter<I: IntoIterator<Item = (&'a str, &'a str)>>(
        iter: I,
    ) -> Result<Self, Self::Error> {
        iter.into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<Self, KeyValuePairError<V::Error>>>()
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

        assert_eq!(label.key, Key::from_str("stackable.tech/vendor").unwrap());
        assert_eq!(label.value, LabelValue::from_str("Stackable").unwrap());

        assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
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

        let labels = Labels::try_from_iter(map).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn labels_to_unvalidated() {
        let labels = Labels::from_iter([
            KeyValuePair::try_from(("stackable.tech/managed-by", "stackablectl")).unwrap(),
            KeyValuePair::try_from(("stackable.tech/vendor", "Stackable")).unwrap(),
        ]);

        let map = labels.to_unvalidated();
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn contains() {
        let labels = label::well_known::sets::common("test", "test-01").unwrap();

        assert!(labels.contains_str_key("app.kubernetes.io/instance"))
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
            merged_labels,
            Labels::try_from_iter([("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")]).unwrap()
        )
    }
}
