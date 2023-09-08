//! This module contains a common [`Duration`] struct which is able to parse
//! human-readable duration formats, like `2y 2h 20m 42s` or`15d 2m 2s`. It
//! additionally implements many required traits, like [`Derivative`],
//! [`JsonSchema`], [`Deserialize`], and [`Serialize`].
//!
//! Furthermore, it implements [`Deref`], which enables us to use all associated
//! functions of [`std::time::Duration`] without re-implementing the public
//! functions on our own type.
//!
//! All operators should opt for [`Duration`] instead of the plain
//! [`std::time::Duration`] when dealing with durations of any form, like
//! timeouts or retries.

use std::{num::ParseIntError, ops::Deref, str::FromStr};

use derivative::Derivative;
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject},
    JsonSchema,
};
use serde::{de::Visitor, Deserialize};
use strum::Display;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DurationParseError {
    #[error("failed to parse string as number")]
    ParseIntError(#[from] ParseIntError),

    #[error("expected a number, found character")]
    ExpectedNumber,

    #[error("expected a character, found number")]
    ExpectedChar,

    #[error("found invalid character")]
    InvalidInput,

    #[error("found invalid unit")]
    InvalidUnit,

    #[error("number overflow")]
    NumberOverflow,
}

/// A [`Duration`] which is capable of parsing human-readable duration formats,
/// like `2y 2h 20m 42s` or `15d 2m 2s`. It additionally provides many required
/// trait implementations, which makes it suited for use in CRDs for example.
#[derive(Clone, Copy, Debug, Derivative, Hash, PartialEq, PartialOrd)]
pub struct Duration(std::time::Duration);

#[derive(Copy, Clone, Debug, Display)]
enum DurationParseState {
    Value,
    Space,
    Init,
    Unit,
    End,
}

impl FromStr for Duration {
    type Err = DurationParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut state = DurationParseState::Init;
        let mut buffer = String::new();
        let mut iter = input.chars();
        let mut is_unit = false;
        let mut cur = 0 as char;

        let mut dur = std::time::Duration::from_secs(0);
        let mut val = 0;

        loop {
            state = match state {
                DurationParseState::Init => match iter.next() {
                    Some(c) => {
                        cur = c;

                        match c {
                            '0'..='9' => DurationParseState::Value,
                            'a'..='z' => DurationParseState::Unit,
                            ' ' => DurationParseState::Space,
                            _ => return Err(DurationParseError::InvalidInput),
                        }
                    }
                    None => DurationParseState::End,
                },
                DurationParseState::Value => {
                    if is_unit {
                        return Err(DurationParseError::ExpectedChar);
                    }

                    buffer.push(cur);
                    DurationParseState::Init
                }
                DurationParseState::Unit => {
                    if !is_unit {
                        is_unit = true;

                        val = buffer.parse::<u64>()?;
                        buffer.clear();
                    }

                    buffer.push(cur);
                    DurationParseState::Init
                }
                DurationParseState::Space => {
                    if !is_unit {
                        return Err(DurationParseError::ExpectedChar);
                    }

                    let factor = parse_unit(&buffer)?;

                    dur = dur
                        .checked_add(std::time::Duration::from_secs(val * factor))
                        .ok_or(DurationParseError::NumberOverflow)?;

                    is_unit = false;
                    buffer.clear();

                    DurationParseState::Init
                }
                DurationParseState::End => {
                    if !is_unit {
                        return Err(DurationParseError::ExpectedChar);
                    }

                    let factor = parse_unit(&buffer)?;

                    dur = dur
                        .checked_add(std::time::Duration::from_secs(val * factor))
                        .ok_or(DurationParseError::NumberOverflow)?;

                    break;
                }
            }
        }

        Ok(Duration(dur))
    }
}

fn parse_unit(buffer: &str) -> Result<u64, DurationParseError> {
    let factor = match buffer {
        "seconds" | "second" | "secs" | "sec" | "s" => 1,
        "minutes" | "minute" | "mins" | "min" | "m" => 60,
        "hours" | "hour" | "hrs" | "hr" | "h" => 3600,
        "days" | "day" | "d" => 86400,
        "weeks" | "week" | "w" => 86400 * 7,
        "months" | "month" | "M" => 2_630_016,
        "years" | "year" | "y" => 31_557_600,
        _ => return Err(DurationParseError::InvalidUnit),
    };

    Ok(factor)
}

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
        let dur = v.parse::<Duration>().map_err(serde::de::Error::custom)?;
        Ok(dur)
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

// impl Serialize for Duration {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         serializer.serialize_str(self.0.to_string().as_str())
//     }
// }

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

// impl Display for Duration {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

impl Deref for Duration {
    type Target = std::time::Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<std::time::Duration> for Duration {
    fn from(value: std::time::Duration) -> Self {
        Self(value)
    }
}

impl Duration {
    /// Creates a new [`Duration`] from the specified number of whole seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(std::time::Duration::from_secs(secs))
    }

    /// Creates a new [`Duration`] from the specified number of milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        Self(std::time::Duration::from_millis(millis))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

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

    // #[test]
    // fn serialize() {
    //     #[derive(Serialize)]
    //     struct S {
    //         dur: Duration,
    //     }

    //     let s = S {
    //         dur: "15d 2m 2s".parse().unwrap(),
    //     };
    //     assert_eq!(serde_yaml::to_string(&s).unwrap(), "dur: 15days 2m 2s\n");
    // }

    // #[test]
    // fn from_impls() {
    //     let dur = Duration::from_secs(10);
    //     assert_eq!(dur.to_string(), "10s");

    //     let dur = Duration::from_millis(1000);
    //     assert_eq!(dur.to_string(), "1s");
    // }
}
