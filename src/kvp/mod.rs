use std::{
    collections::{BTreeMap, HashSet},
    fmt::Display,
    ops::Deref,
    str::FromStr,
};

use snafu::{ensure, ResultExt, Snafu};

mod key;
mod serde_impl;
mod value;

pub use key::*;
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

pub type Annotations = KeyValuePairs<AnnotationValue>;
pub type Annotation = KeyValuePair<AnnotationValue>;

pub type Labels = KeyValuePairs<LabelValue>;
pub type Label = KeyValuePair<LabelValue>;

/// A [`KeyValuePair`] is a pair values which consist of a [`Key`] and value.
/// These pairs can be used as Kubernetes labels or annotations. A pair can be
/// parsed from a string with the following format: `(<PREFIX>/)<NAME>=<VALUE>`.
///
/// ### Links
///
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
#[derive(Debug, PartialEq, Eq, Hash)]
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

impl<T, V> TryFrom<(T, T)> for KeyValuePair<V>
where
    T: AsRef<str>,
    V: ValueExt,
{
    type Error = KeyValuePairError<V::Error>;

    fn try_from(value: (T, T)) -> Result<Self, Self::Error> {
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
pub enum KeyValuePairsError {
    AlreadyPresent,
}

/// [`KeyValuePairs`] is a list of [`KeyValuePair`]. This collection **doesn't**
/// provide any de-duplication mechanism, meaning multiple [`KeyValuePair`]s
/// with the same content can be present at the same time. However, converting
/// to a [`BTreeMap<String, String>`] removes any duplicate data. Order matters
/// in this case: later labels overwrite previous onces.
#[derive(Debug, Default)]
pub struct KeyValuePairs<V: ValueExt>(HashSet<KeyValuePair<V>>);

impl<V> TryFrom<BTreeMap<String, String>> for KeyValuePairs<V>
where
    V: ValueExt,
{
    type Error = KeyValuePairError<V::Error>;

    fn try_from(map: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let pairs = map
            .into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<HashSet<_>, KeyValuePairError<V::Error>>>()?;

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
    type Target = HashSet<KeyValuePair<V>>;

    fn deref(&self) -> &Self::Target {
        &self.0
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
    pub fn new_with(pairs: HashSet<KeyValuePair<V>>) -> Self {
        Self(pairs)
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    pub fn try_insert(&mut self, kvp: KeyValuePair<V>) -> Result<&mut Self, KeyValuePairsError> {
        ensure!(!self.contains(&kvp), AlreadyPresentSnafu);

        self.0.insert(kvp);
        Ok(self)
    }

    pub fn insert(&mut self, kvp: KeyValuePair<V>) -> &mut Self {
        self.0.insert(kvp);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

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
    fn pairs_from_iter() {
        let labels = Labels::from_iter([
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
        ]);

        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn pairs_try_from_map() {
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
    fn pairs_into_map() {
        let pairs = HashSet::from([
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
        ]);

        let labels = Labels::new_with(pairs);
        let map: BTreeMap<String, String> = labels.into();

        assert_eq!(map.len(), 2);
    }
}
