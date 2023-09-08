//! This module contains a common [`Duration`] struct which is able to parse
//! human-readable duration formats, like `2y 2h 20m 42s`, `15d 2m 2s`, or
//! `2018-01-01T12:53:00Z` defined by [RFC 3339][1]. The [`Duration`] exported
//! in this module doesn't provide the parsing logic by itself, but instead
//! uses the crate [humantime]. It additionally implements many required
//! traits, like [`Derivative`], [`JsonSchema`], [`Deserialize`], and
//! [`Serialize`].
//!
//! Furthermore, it implements [`Deref`], which enables us to use all associated
//! functions of [`humantime::Duration`] without re-implementing the public
//! functions on our own type.
//!
//! All operators should opt for [`Duration`] instead of the plain
//! [`std::time::Duration`] when dealing with durations of any form, like
//! timeouts or retries.
//!
//! [1]: https://www.rfc-editor.org/rfc/rfc3339

use std::{fmt::Display, ops::Deref, str::FromStr};

use derivative::Derivative;
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject},
    JsonSchema,
};
use serde::{de::Visitor, Deserialize, Serialize};

/// A [`Duration`] which is capable of parsing human-readable duration formats,
/// like `2y 2h 20m 42s`, `15d 2m 2s`, or `2018-01-01T12:53:00Z` defined by
/// [RFC 3339][1]. It additionally provides many required trait implementations,
/// which makes it suited for use in CRDs for example.
///
/// [1]: https://www.rfc-editor.org/rfc/rfc3339
#[derive(Clone, Copy, Debug, Derivative, Hash, PartialEq)]
pub struct Duration(humantime::Duration);

struct DurationVisitor;

impl<'de> Visitor<'de> for DurationVisitor {
    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string in any of the supported formats")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let dur = v
            .parse::<humantime::Duration>()
            .map_err(serde::de::Error::custom)?;

        Ok(Duration(dur))
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(DurationVisitor)
    }
}

impl Serialize for Duration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.to_string().as_str())
    }
}

impl JsonSchema for Duration {
    fn schema_name() -> String {
        "Duration".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl FromStr for Duration {
    type Err = humantime::DurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<humantime::Duration>()?))
    }
}

impl PartialOrd for Duration {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for Duration {
    type Target = humantime::Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Duration {
    /// Creates a new [`Duration`] from the specified number of whole seconds.
    pub fn from_secs(secs: u64) -> Self {
        Self(std::time::Duration::from_secs(secs).into())
    }

    /// Creates a new [`Duration`] from the specified number of milliseconds.
    pub fn from_millis(millis: u64) -> Self {
        Self(std::time::Duration::from_millis(millis).into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[test]
    fn deref() {
        let dur: Duration = "1h".parse().unwrap();
        assert_eq!(dur.as_secs(), 3600);
    }

    #[rstest]
    #[case("2y 2h 20m 42s", 63123642)]
    #[case("15d 2m 2s", 1296122)]
    #[case("1h", 3600)]
    #[case("1m", 60)]
    #[case("1s", 1)]
    fn parse(#[case] input: &str, #[case] output: u64) {
        let dur: Duration = input.parse().unwrap();
        assert_eq!(dur.as_secs(), output);
    }

    #[test]
    fn deserialize() {
        #[derive(Deserialize)]
        struct S {
            dur: Duration,
        }

        let s: S = serde_yaml::from_str("dur: \"15d 2m 2s\"").unwrap();
        assert_eq!(s.dur.as_secs(), 1296122);
    }

    #[test]
    fn serialize() {
        #[derive(Serialize)]
        struct S {
            dur: Duration,
        }

        let s = S {
            dur: "15d 2m 2s".parse().unwrap(),
        };
        assert_eq!(serde_yaml::to_string(&s).unwrap(), "dur: 15days 2m 2s\n");
    }

    #[test]
    fn from_impls() {
        let dur = Duration::from_secs(10);
        assert_eq!(dur.to_string(), "10s");

        let dur = Duration::from_millis(1000);
        assert_eq!(dur.to_string(), "1s");
    }
}
