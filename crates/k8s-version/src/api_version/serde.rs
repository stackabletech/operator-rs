use std::str::FromStr;

use serde::{Deserialize, Serialize, de::Visitor};

use crate::ApiVersion;

impl<'de> Deserialize<'de> for ApiVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ApiVersionVisitor;

        impl Visitor<'_> for ApiVersionVisitor {
            type Value = ApiVersion;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid Kubernetes API version")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ApiVersion::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(ApiVersionVisitor)
    }
}

impl Serialize for ApiVersion {
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
        let _: ApiVersion =
            serde_yaml::from_str("extensions.k8s.io/v1alpha1").expect("api version is valid");
    }

    #[test]
    fn serialize() {
        let api_version =
            ApiVersion::from_str("extensions.k8s.io/v1alpha1").expect("api version is valid");
        assert_eq!(
            "extensions.k8s.io/v1alpha1\n",
            serde_yaml::to_string(&api_version).expect("api version must serialize")
        );
    }
}
