use std::str::FromStr;

use serde::{Deserialize, Serialize, de::Visitor};

use crate::Level;

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LevelVisitor;

        impl<'de> Visitor<'de> for LevelVisitor {
            type Value = Level;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid Kubernetes API version level")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Level::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(LevelVisitor)
    }
}

impl Serialize for Level {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize() {
        let _: Level = serde_yaml::from_str("alpha1").expect("level is valid");
    }

    #[test]
    fn serialize() {
        let api_version = Level::from_str("alpha1").expect("level is valid");
        assert_eq!(
            "alpha1\n",
            serde_yaml::to_string(&api_version).expect("level must serialize")
        );
    }
}
