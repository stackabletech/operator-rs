use serde::{ser::SerializeMap, Serialize};

use crate::kvp::{KeyValuePair, KeyValuePairs, ValueExt};

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
