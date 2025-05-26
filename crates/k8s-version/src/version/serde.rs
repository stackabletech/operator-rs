use std::str::FromStr;

use serde::{Deserialize, Serialize, de::Visitor};

use crate::Version;

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct VersionVisitor;

        impl Visitor<'_> for VersionVisitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid Kubernetes API version")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Version::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(VersionVisitor)
    }
}

impl Serialize for Version {
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
        let _: Version = serde_yaml::from_str("v1alpha1").expect("version is valid");
    }

    #[test]
    fn serialize() {
        let api_version = Version::from_str("v1alpha1").expect("version is valid");
        assert_eq!(
            "v1alpha1\n",
            serde_yaml::to_string(&api_version).expect("version must serialize")
        );
    }
}
