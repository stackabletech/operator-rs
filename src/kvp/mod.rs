use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    marker::PhantomData,
    ops::Deref,
    str::FromStr,
};

use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};
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

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyValuePairError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("label input cannot be empty"))]
    EmptyInput,

    #[snafu(display("invalid number of equal signs - expected exactly 1, got {signs}"))]
    InvalidEqualSignCount { signs: usize },

    #[snafu(display("failed to parse label key"))]
    InvalidKey { source: KeyError },

    #[snafu(display("failed to parse label value"))]
    InvalidValue { source: E },
}

/// A [`KeyValuePair`] is a pair values which consist of a [`Key`] and value.
/// These pairs can be used as Kubernetes labels or annotations. A pair can be
/// parsed from a string with the following format: `(<PREFIX>/)<NAME>=<VALUE>`.
///
/// ### Links
///
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyValuePair<V>
where
    V: ValueExt,
{
    key: Key,
    value: V,
}

impl<V> FromStr for KeyValuePair<V>
where
    V: ValueExt,
{
    type Err = KeyValuePairError<V::Error>;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();

        // Ensure the input is not empty
        ensure!(!input.is_empty(), EmptyInputSnafu);

        // Then split up the key and value, which is separated by an equal
        // sign
        let parts: Vec<_> = input.split('=').collect();

        // Ensure there are only two parts
        ensure!(
            parts.len() == 2,
            InvalidEqualSignCountSnafu {
                signs: parts.len() - 1
            }
        );

        // Parse key and value parts
        let key = Key::from_str(parts[0]).context(InvalidKeySnafu)?;
        let value = V::from_str(parts[1]).context(InvalidValueSnafu)?;

        Ok(Self { key, value })
    }
}

impl<T, K, V> TryFrom<(T, K)> for KeyValuePair<V>
where
    T: AsRef<str>,
    K: AsRef<str>,
    V: ValueExt,
{
    type Error = KeyValuePairError<V::Error>;

    fn try_from(value: (T, K)) -> Result<Self, Self::Error> {
        let key = Key::from_str(value.0.as_ref()).context(InvalidKeySnafu)?;
        let value = V::from_str(value.1.as_ref()).context(InvalidValueSnafu)?;

        Ok(Self { key, value })
    }
}

impl<V> Display for KeyValuePair<V>
where
    V: ValueExt,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl<V> Serialize for KeyValuePair<V>
where
    V: ValueExt,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.key, &self.value)?;
        map.end()
    }
}

struct KeyValuePairVisitor<V> {
    marker: PhantomData<V>,
}

impl<'de, V> Visitor<'de> for KeyValuePairVisitor<V>
where
    V: Deserialize<'de> + ValueExt + Default,
{
    type Value = KeyValuePair<V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid key/value pair (label or annotation)")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        if let Some((key, value)) = map.next_entry()? {
            return Ok(KeyValuePair::new(key, value));
        }

        Err(serde::de::Error::custom("expected at least one map entry"))
    }
}

impl<'de, V> Deserialize<'de> for KeyValuePair<V>
where
    V: Deserialize<'de> + ValueExt + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(KeyValuePairVisitor {
            marker: PhantomData,
        })
    }
}

impl<V> KeyValuePair<V>
where
    V: ValueExt,
{
    /// Creates a new [`KeyValuePair`] from a validated [`Key`] and value.
    pub fn new(key: Key, value: V) -> Self {
        Self { key, value }
    }

    /// Returns an immutable reference to the pair's [`Key`].
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Returns an immutable reference to the pair's value.
    pub fn value(&self) -> &V {
        &self.value
    }
}

#[derive(Debug, Snafu)]
pub enum KeyValuePairsError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("key/value pair already present"))]
    AlreadyPresent,

    #[snafu(display("failed to parse key/value pair"))]
    KeyValuePairParse { source: KeyValuePairError<E> },
}

/// [`KeyValuePairs`] is a list of [`KeyValuePair`]. This collection **doesn't**
/// provide any de-duplication mechanism, meaning multiple [`KeyValuePair`]s
/// with the same content can be present at the same time. However, converting
/// to a [`BTreeMap<String, String>`] removes any duplicate data. Order matters
/// in this case: later labels overwrite previous onces.
#[derive(Clone, Debug, Default)]
pub struct KeyValuePairs<V: ValueExt>(BTreeSet<KeyValuePair<V>>);

impl<V> TryFrom<BTreeMap<String, String>> for KeyValuePairs<V>
where
    V: ValueExt,
{
    type Error = KeyValuePairError<V::Error>;

    fn try_from(map: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let pairs = map
            .into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<BTreeSet<_>, KeyValuePairError<V::Error>>>()?;

        Ok(Self(pairs))
    }
}

impl<V> FromIterator<KeyValuePair<V>> for KeyValuePairs<V>
where
    V: ValueExt,
{
    fn from_iter<T: IntoIterator<Item = KeyValuePair<V>>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<V> From<KeyValuePairs<V>> for BTreeMap<String, String>
where
    V: ValueExt,
{
    fn from(value: KeyValuePairs<V>) -> Self {
        value
            .iter()
            .map(|pair| (pair.key().to_string(), pair.value().to_string()))
            .collect()
    }
}

impl<V> Deref for KeyValuePairs<V>
where
    V: ValueExt,
{
    type Target = BTreeSet<KeyValuePair<V>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V> Serialize for KeyValuePairs<V>
where
    V: ValueExt,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for kvp in &self.0 {
            map.serialize_entry(kvp.key(), kvp.value())?;
        }
        map.end()
    }
}

struct KeyValuePairsVisitor<V> {
    value_marker: PhantomData<V>,
}

impl<V> KeyValuePairsVisitor<V> {
    pub fn new() -> Self {
        Self {
            value_marker: PhantomData,
        }
    }
}

impl<'de, V> Visitor<'de> for KeyValuePairsVisitor<V>
where
    V: Deserialize<'de> + ValueExt + Default,
{
    type Value = KeyValuePairs<V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("valid list of key/value pairs (labels and or annotations)")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut pairs = KeyValuePairs::new();
        while let Some((key, value)) = map.next_entry()? {
            pairs.insert(KeyValuePair::new(key, value));
        }
        Ok(pairs)
    }
}

impl<'de, V> Deserialize<'de> for KeyValuePairs<V>
where
    V: Deserialize<'de> + ValueExt + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(KeyValuePairsVisitor::new())
    }
}

impl<V> KeyValuePairs<V>
where
    V: ValueExt + std::default::Default,
{
    /// Creates a new empty list of [`KeyValuePair`]s.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`KeyValuePair`]s from `pairs`.
    pub fn new_with(pairs: BTreeSet<KeyValuePair<V>>) -> Self {
        Self(pairs)
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    pub fn try_insert(
        &mut self,
        kvp: KeyValuePair<V>,
    ) -> Result<&mut Self, KeyValuePairsError<V::Error>> {
        ensure!(!self.0.contains(&kvp), AlreadyPresentSnafu);

        self.0.insert(kvp);
        Ok(self)
    }

    pub fn insert(&mut self, kvp: KeyValuePair<V>) -> &mut Self {
        self.0.insert(kvp);
        self
    }

    pub fn contains(&self, kvp: &KeyValuePair<V>) -> bool {
        self.0.contains(kvp)
    }

    pub fn contains_raw(
        &self,
        key: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<bool, KeyValuePairsError<V::Error>> {
        let kvp = KeyValuePair::try_from((key.as_ref(), value.as_ref()))
            .context(KeyValuePairParseSnafu)?;

        Ok(self.0.contains(&kvp))
    }

    pub fn contains_all(&self, kvps: KeyValuePairs<V>) -> bool {
        for kvp in kvps.iter() {
            if !self.contains(kvp) {
                return false;
            }
        }

        true
    }

    pub fn contains_all_raw<'a>(
        &self,
        keys: impl AsRef<[&'a str]>,
        values: impl AsRef<[&'a str]>,
    ) -> Result<bool, KeyValuePairsError<V::Error>> {
        for (key, value) in keys.as_ref().iter().zip(values.as_ref()) {
            if !self.contains_raw(key, value)? {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

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
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    #[rstest]
    #[case(
        "stackable.tech/managed-by=stackablectl",
        "stackable.tech/managed-by",
        "stackablectl"
    )]
    #[case(
        "stackable.tech/vendor=Stackable",
        "stackable.tech/vendor",
        "Stackable"
    )]
    #[case("foo=bar", "foo", "bar")]
    fn from_str_valid(#[case] input: &str, #[case] key: &str, #[case] value: &str) {
        let label = Label::from_str(input).unwrap();

        assert_eq!(label.key(), &Key::from_str(key).unwrap());
        assert_eq!(label.value(), &LabelValue::from_str(value).unwrap());
    }

    #[rstest]
    #[case("foo=bar=baz", KeyValuePairError::InvalidEqualSignCount { signs: 2 })]
    #[case("", KeyValuePairError::EmptyInput)]
    fn from_str_invalid(#[case] input: &str, #[case] error: KeyValuePairError<LabelValueError>) {
        let err = Label::from_str(input).unwrap_err();
        assert_eq!(err, error)
    }

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
    fn labels_from_iter() {
        let labels = Labels::from_iter([
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
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
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
        ]);

        let labels = Labels::new_with(pairs);
        let map: BTreeMap<String, String> = labels.into();

        assert_eq!(map.len(), 2);
    }

    #[test]
    fn serialize() {
        #[derive(Serialize)]
        struct Kvp {
            labels: Labels,
            label: Label,
        }

        let label = Label::from_str("stackable.tech/managed-by=stackablectl").unwrap();
        let labels = Labels::common("zookeeper", "zookeeper-default-1").unwrap();

        let kvp = Kvp { labels, label };

        assert_eq!(serde_yaml::to_string(&kvp).unwrap(), "labels:\n  app.kubernetes.io/instance: zookeeper-default-1\n  app.kubernetes.io/name: zookeeper\nlabel:\n  stackable.tech/managed-by: stackablectl\n");
    }

    #[test]
    fn deserialize() {
        #[derive(Deserialize)]
        struct Kvp {
            labels: Labels,
            label: Label,
        }

        let kvp: Kvp = serde_yaml::from_str("labels:\n  app.kubernetes.io/instance: zookeeper-default-1\n  app.kubernetes.io/name: zookeeper\nlabel:\n  stackable.tech/managed-by: stackablectl\n").unwrap();

        assert_eq!(kvp.label.key().to_string(), "stackable.tech/managed-by");
        assert_eq!(kvp.label.value().to_string(), "stackablectl");

        assert_eq!(kvp.labels.len(), 2);
    }
}
