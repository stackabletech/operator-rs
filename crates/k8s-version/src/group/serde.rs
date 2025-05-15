use std::str::FromStr;

use serde::{Deserialize, Serialize, de::Visitor};

use crate::Group;

impl<'de> Deserialize<'de> for Group {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct GroupVisitor;

        impl<'de> Visitor<'de> for GroupVisitor {
            type Value = Group;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid Kubernetes API group")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Group::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(GroupVisitor)
    }
}

impl Serialize for Group {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self)
    }
}
