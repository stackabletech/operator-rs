use std::{marker::PhantomData, str::FromStr};

use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};

use crate::kvp::{Key, KeyValuePair, KeyValuePairs, ValueExt};

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KeyVisitor;

        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = Key;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid kubernetes label or annotation key")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Key::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(KeyVisitor)
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
    V: ValueExt,
{
    type Value = KeyValuePair<V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid key/value pair (label or annotation)")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        KeyValuePair::from_str(v).map_err(serde::de::Error::custom)
    }
}

impl<'de, V> Deserialize<'de> for KeyValuePair<V>
where
    V: ValueExt,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(KeyValuePairVisitor {
            marker: PhantomData,
        })
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
