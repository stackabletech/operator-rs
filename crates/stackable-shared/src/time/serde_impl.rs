use serde::{Deserialize, Serialize, de::Visitor};

use crate::time::{Duration, DurationWithNever};

struct DurationVisitor;

impl Visitor<'_> for DurationVisitor {
    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string in any of the supported formats")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let dur = v.parse::<Duration>().map_err(serde::de::Error::custom)?;
        Ok(dur)
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(DurationVisitor)
    }
}

impl Serialize for Duration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct DurationWithNeverVisitor;

impl Visitor<'_> for DurationWithNeverVisitor {
    type Value = DurationWithNever;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_fmt(format_args!(
            "the string \"{keyword}\" or a string in any of the supported formats",
            keyword = DurationWithNever::NEVER_KEYWORD
        ))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let dur = v
            .parse::<DurationWithNever>()
            .map_err(serde::de::Error::custom)?;
        Ok(dur)
    }
}

impl<'de> Deserialize<'de> for DurationWithNever {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(DurationWithNeverVisitor)
    }
}

impl Serialize for DurationWithNever {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
