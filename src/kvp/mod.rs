use std::{collections::BTreeMap, fmt::Display, ops::Deref, str::FromStr};

use snafu::{ensure, ResultExt, Snafu};

mod key;
mod value;

pub use key::*;
pub use value::*;

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyValuePairError {
    #[snafu(display("label input cannot be empty"))]
    EmptyInput,

    #[snafu(display("invalid number of equal signs - expected exactly 1, got {signs}"))]
    InvalidEqualSignCount { signs: usize },

    #[snafu(display("failed to parse label key"))]
    KeyError { source: KeyError },

    #[snafu(display("failed to parse label value"))]
    ValueError { source: ValueError },
}

/// A [`KeyValuePair`] is a pair values which consist of a [`Key`] and [`Value`].
/// These pairs can be used as Kubernetes labels or annotations. A pair can be
/// parsed from a string with the following format: `(<PREFIX>/)<NAME>=<VALUE>`.
///
/// ### Links
///
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/>
/// - <https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/>
#[derive(Debug)]
pub struct KeyValuePair {
    key: Key,
    value: Value,
}

impl FromStr for KeyValuePair {
    type Err = KeyValuePairError;

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
        let key = Key::from_str(parts[0]).context(KeySnafu)?;
        let value = Value::from_str(parts[1]).context(ValueSnafu)?;

        Ok(Self { key, value })
    }
}

impl<T> TryFrom<(T, T)> for KeyValuePair
where
    T: AsRef<str>,
{
    type Error = KeyValuePairError;

    fn try_from(value: (T, T)) -> Result<Self, Self::Error> {
        let key = Key::from_str(value.0.as_ref()).context(KeySnafu)?;
        let value = Value::from_str(value.1.as_ref()).context(ValueSnafu)?;

        Ok(Self { key, value })
    }
}

impl Display for KeyValuePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl KeyValuePair {
    /// Creates a new [`KeyValuePair`] from a validated [`Key`] and [`Value`].
    pub fn new(key: Key, value: Value) -> Self {
        Self { key, value }
    }

    /// Returns an immutable reference to the pair's [`Key`].
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Returns an immutable reference to the pair's [`Value`].
    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// [`KeyValuePairs`] is a list of [`KeyValuePair`]s. It provides different helper
/// functions to convert from and to [`BTreeMap<String, String>`] and
/// [`Vec<KeyValuePair>`].
#[derive(Debug, Default)]
struct KeyValuePairs(Vec<KeyValuePair>);

impl TryFrom<BTreeMap<String, String>> for KeyValuePairs {
    type Error = KeyValuePairError;

    fn try_from(map: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let pairs = map
            .into_iter()
            .map(KeyValuePair::try_from)
            .collect::<Result<Vec<_>, KeyValuePairError>>()?;

        Ok(Self(pairs))
    }
}

impl FromIterator<KeyValuePair> for KeyValuePairs {
    fn from_iter<T: IntoIterator<Item = KeyValuePair>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl From<KeyValuePairs> for BTreeMap<String, String> {
    fn from(value: KeyValuePairs) -> Self {
        value
            .iter()
            .map(|pair| (pair.key().to_string(), pair.value().to_string()))
            .collect()
    }
}

impl Deref for KeyValuePairs {
    type Target = Vec<KeyValuePair>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl KeyValuePairs {
    /// Creates a new empty list of [`KeyValuePair`]s.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new list of [`KeyValuePair`]s from `pairs`.
    pub fn new_with(pairs: Vec<KeyValuePair>) -> Self {
        Self(pairs)
    }

    /// Extends `self` with `other`.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// Adds a [`KeyValuePair`] to the end of the list of [`KeyValuePairs`].
    pub fn push(&mut self, key: &str, value: &str) -> Result<&mut Self, KeyValuePairError> {
        let key = Key::from_str(key).context(KeySnafu)?;
        let value = Value::from_str(value).context(ValueSnafu)?;

        self.0.push(KeyValuePair::new(key, value));
        Ok(self)
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
        let kvp = KeyValuePair::from_str(input).unwrap();

        assert_eq!(kvp.key(), &Key::from_str(key).unwrap());
        assert_eq!(kvp.value(), &Value::from_str(value).unwrap());
    }

    #[rstest]
    #[case("foo=bar=baz", KeyValuePairError::InvalidEqualSignCount { signs: 2 })]
    #[case("", KeyValuePairError::EmptyInput)]
    fn from_str_invalid(#[case] input: &str, #[case] error: KeyValuePairError) {
        let err = KeyValuePair::from_str(input).unwrap_err();
        assert_eq!(err, error)
    }

    #[test]
    fn from_iter() {
        let kvps = KeyValuePairs::from_iter([
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
        ]);

        assert_eq!(kvps.len(), 2);
    }

    #[test]
    fn try_from_tuple() {
        let kvp = KeyValuePair::try_from(("stackable.tech/vendor", "Stackable")).unwrap();

        assert_eq!(kvp.key(), &Key::from_str("stackable.tech/vendor").unwrap());
        assert_eq!(kvp.value(), &Value::from_str("Stackable").unwrap());

        assert_eq!(kvp.to_string(), "stackable.tech/vendor=Stackable");
    }

    #[test]
    fn try_from_map() {
        let map = BTreeMap::from([
            ("stackable.tech/vendor".to_string(), "Stackable".to_string()),
            (
                "stackable.tech/managed-by".to_string(),
                "stackablectl".to_string(),
            ),
        ]);

        let kvps = KeyValuePairs::try_from(map).unwrap();
        assert_eq!(kvps.len(), 2);
    }

    #[test]
    fn into_map() {
        let pairs = vec![
            KeyValuePair::from_str("stackable.tech/vendor=Stackable").unwrap(),
            KeyValuePair::from_str("stackable.tech/managed-by=stackablectl").unwrap(),
        ];

        let kvps = KeyValuePairs::new_with(pairs);
        let map: BTreeMap<String, String> = kvps.into();

        assert_eq!(map.len(), 2);
    }
}
