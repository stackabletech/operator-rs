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

use std::{
    fmt::{Display, Write},
    num::ParseIntError,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use derivative::Derivative;
use schemars::JsonSchema;
use strum::IntoEnumIterator;
use thiserror::Error;

mod serde_impl;
pub use serde_impl::*;

#[derive(Debug, Error, PartialEq)]
pub enum DurationParseError {
    #[error("invalid input, either empty or contains non-ascii characters")]
    InvalidInput,

    #[error("failed to parse duration fragment")]
    FragmentError(#[from] DurationFragmentParseError),
}

#[derive(Clone, Copy, Debug, Derivative, Hash, PartialEq, PartialOrd, JsonSchema)]
pub struct Duration(std::time::Duration);

impl FromStr for Duration {
    type Err = DurationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = s.trim();

        // An empty or non-ascii input is invalid
        if input.is_empty() || !input.is_ascii() {
            return Err(DurationParseError::InvalidInput);
        }

        // Let's split up individual parts separated by a space
        let parts: Vec<&str> = input.split(' ').collect();

        // Parse each part as a DurationFragment and extract the final duration
        // of each fragment in milliseconds
        let values: Vec<u128> = parts
            .iter()
            .map(|p| p.parse::<DurationFragment>())
            .map(|r| r.map(|f| f.millis()))
            .collect::<Result<Vec<_>, DurationFragmentParseError>>()?;

        // NOTE (Techassi): This derefernce is super weird, but
        // Duration::from_millis doesn't accept a u128, but returns a u128
        // when as_millis is called.
        Ok(Self(std::time::Duration::from_millis(
            values.iter().fold(0, |acc, v| acc + (*v as u64)),
        )))
    }
}

impl Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // If the inner Duration is zero, print out '0ms' as milliseconds
        // is the base unit for our Duration.
        if self.0.is_zero() {
            return write!(f, "0{}", DurationUnit::Milliseconds);
        }

        let mut millis = self.0.as_millis();
        let mut formatted = String::new();

        for unit in DurationUnit::iter() {
            let whole = millis / unit.millis();
            let rest = millis % unit.millis();

            if whole > 0 {
                write!(formatted, "{}{} ", whole, unit)?;
            }

            millis = rest;
        }

        write!(f, "{}", formatted.trim_end())
    }
}

impl Deref for Duration {
    type Target = std::time::Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Duration {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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

    /// Creates a new [`Duration`] from the specified number of whole
    /// milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        Self(std::time::Duration::from_millis(millis))
    }
}

/// Defines supported [`DurationUnit`]s. Each [`DurationFragment`] consists of
/// a numeric value followed by a [`DurationUnit`]. The order of variants
/// **MATTERS**. It is the basis for the correct transformation of the
/// [`std::time::Duration`] back to a human-readable format, which is defined
/// in the [`Display`] implementation of [`Duration`].
#[derive(Debug, strum::EnumString, strum::Display, strum::AsRefStr, strum::EnumIter)]
pub enum DurationUnit {
    #[strum(serialize = "d")]
    Days,

    #[strum(serialize = "h")]
    Hours,

    #[strum(serialize = "m")]
    Minutes,

    #[strum(serialize = "s")]
    Seconds,

    #[strum(serialize = "ms")]
    Milliseconds,
}

impl DurationUnit {
    /// Returns the number of whole milliseconds in each supported
    /// [`DurationUnit`].
    pub fn millis(&self) -> u128 {
        use DurationUnit::*;

        match self {
            Days => 24 * Hours.millis(),
            Hours => 60 * Minutes.millis(),
            Minutes => 60 * Seconds.millis(),
            Seconds => 1000,
            Milliseconds => 1,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum DurationFragmentParseError {
    #[error("invalid input, either empty or contains non-ascii characters")]
    InvalidInput,

    #[error("expected number, the duration fragment must start with a numeric character")]
    ExpectedNumber,

    #[error("expected character, the duration fragments must end with an alphabetic character")]
    ExpectedCharacter,

    #[error("failed to parse fragment value as integer")]
    ParseIntError(#[from] ParseIntError),

    #[error("failed to parse fragment unit")]
    UnitParseError,
}

/// Each [`DurationFragment`] consists of a numeric value followed by
/// a[`DurationUnit`].
#[derive(Debug)]
pub struct DurationFragment {
    value: u128,
    unit: DurationUnit,
}

impl FromStr for DurationFragment {
    type Err = DurationFragmentParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = s.trim();

        // An empty is invalid, non-ascii characters are already ruled out by
        // the Duration impl
        if input.is_empty() {
            return Err(DurationFragmentParseError::InvalidInput);
        }

        let mut chars = input.char_indices().peekable();
        let mut end_index = 0;

        // First loop through all numeric characters
        while let Some((i, _)) = chars.next_if(|(_, c)| char::is_numeric(*c)) {
            end_index = i + 1;
        }

        // Parse the numeric characters as a u128
        let value = if end_index != 0 {
            s[0..end_index].parse::<u128>()?
        } else {
            return Err(DurationFragmentParseError::ExpectedNumber);
        };

        // Loop through all alphabetic characters
        let start_index = end_index;
        while let Some((i, _)) = chars.next_if(|(_, c)| char::is_alphabetic(*c)) {
            end_index = i + 1;
        }

        // Parse the alphabetic characters as a supported duration unit
        let unit = if end_index != 0 {
            s[start_index..end_index]
                .parse::<DurationUnit>()
                .map_err(|_| DurationFragmentParseError::UnitParseError)?
        } else {
            return Err(DurationFragmentParseError::ExpectedCharacter);
        };

        // If there are characters left which are not alphabetic, we return an
        // error
        if chars.peek().is_some() {
            return Err(DurationFragmentParseError::ExpectedCharacter);
        }

        Ok(Self { value, unit })
    }
}

impl Display for DurationFragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

impl DurationFragment {
    /// Returns the amount of whole milliseconds encoded by this
    /// [`DurationFragment`].
    pub fn millis(&self) -> u128 {
        self.value * self.unit.millis()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    #[rstest]
    #[case("15d 2m 2s", 1296122)]
    #[case("1h", 3600)]
    #[case("1m", 60)]
    #[case("1s", 1)]
    fn parse(#[case] input: &str, #[case] output: u64) {
        let dur: Duration = input.parse().unwrap();
        assert_eq!(dur.as_secs(), output);
    }

    #[rstest]
    #[case(
        "2d2",
        DurationParseError::FragmentError(DurationFragmentParseError::ExpectedCharacter)
    )]
    #[case(
        "-1y",
        DurationParseError::FragmentError(DurationFragmentParseError::ExpectedNumber)
    )]
    #[case(
        "1D",
        DurationParseError::FragmentError(DurationFragmentParseError::UnitParseError)
    )]
    #[case("1ä", DurationParseError::InvalidInput)]
    #[case(" ", DurationParseError::InvalidInput)]
    fn parse_invalid(#[case] input: &str, #[case] expected_err: DurationParseError) {
        let err = Duration::from_str(input).unwrap_err();
        assert_eq!(err, expected_err)
    }

    #[rstest]
    #[case("15d 2m 2s")]
    #[case("1h 20m")]
    #[case("1m")]
    #[case("1s")]
    fn to_string(#[case] duration: &str) {
        let dur: Duration = duration.parse().unwrap();
        assert_eq!(dur.to_string(), duration);
    }

    #[test]
    fn deserialize() {
        #[derive(Deserialize)]
        struct S {
            dur: Duration,
        }

        let s: S = serde_yaml::from_str("dur: 15d 2m 2s").unwrap();
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
        assert_eq!(serde_yaml::to_string(&s).unwrap(), "dur: 15d 2m 2s\n");
    }

    // #[test]
    // fn add_ops() {
    //     let mut dur1 = Duration::from_secs(20);
    //     let dur2 = Duration::from_secs(10);

    //     let dur = dur1 + dur2;
    //     assert_eq!(dur.as_secs(), 30);

    //     dur1 += dur2;
    //     assert_eq!(dur1.as_secs(), 30);
    // }

    // #[test]
    // fn sub_ops() {
    //     let mut dur1 = Duration::from_secs(20);
    //     let dur2 = Duration::from_secs(10);

    //     let dur = dur1 - dur2;
    //     assert_eq!(dur.as_secs(), 10);

    //     dur1 -= dur2;
    //     assert_eq!(dur1.as_secs(), 10);
    // }
}
