//! This module contains a common [`Duration`] struct which is able to parse
//! human-readable duration formats, like `5s`, `24h`, `2y2h20m42s` or`15d2m2s`. It
//! additionally implements many required traits, like [`Derivative`],
//! [`JsonSchema`], [`Deserialize`][serde::Deserialize], and
//! [`Serialize`][serde::Serialize].
//!
//! Furthermore, it implements [`Deref`], which enables us to use all associated
//! functions of [`std::time::Duration`] without re-implementing the public
//! functions on our own type.
//!
//! All operators should opt for [`Duration`] instead of the plain
//! [`std::time::Duration`] when dealing with durations of any form, like
//! timeouts or retries.

use std::{
    cmp::Ordering,
    fmt::Display,
    num::ParseIntError,
    ops::{Add, AddAssign, Deref, DerefMut, Div, Mul, Sub, SubAssign},
    str::FromStr,
};

use derivative::Derivative;
use schemars::JsonSchema;
use snafu::{OptionExt, ResultExt, Snafu};
use strum::IntoEnumIterator;

mod serde_impl;

#[derive(Debug, Snafu, PartialEq)]
#[snafu(module)]
pub enum DurationParseError {
    #[snafu(display("invalid input, either empty or contains non-ascii characters"))]
    InvalidInput,

    #[snafu(display("unexpected character {chr:?}"))]
    UnexpectedCharacter { chr: char },

    #[snafu(display("fragment with value {value:?} has no unit"))]
    NoUnit { value: u128 },

    #[snafu(display("invalid fragment order, {current} must be before {previous}"))]
    InvalidUnitOrdering {
        previous: DurationUnit,
        current: DurationUnit,
    },

    #[snafu(display("fragment unit {unit} was specified multiple times"))]
    DuplicateUnit { unit: DurationUnit },

    #[snafu(display("failed to parse fragment unit {unit:?}"))]
    ParseUnitError { unit: String },

    #[snafu(display("failed to parse fragment value as integer"))]
    ParseIntError { source: ParseIntError },
}

#[derive(Clone, Copy, Debug, Derivative, Hash, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub struct Duration(std::time::Duration);

impl FromStr for Duration {
    type Err = DurationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use duration_parse_error::*;
        let input = s.trim();

        // An empty or non-ascii input is invalid
        if input.is_empty() || !input.is_ascii() {
            return Err(DurationParseError::InvalidInput);
        }

        let mut chars = input.char_indices().peekable();
        let mut duration = std::time::Duration::ZERO;
        let mut last_unit = None;

        let mut take_group = |f: fn(char) -> bool| {
            let &(from, _) = chars.peek()?;
            let mut to = from;

            while let Some((i, _)) = chars.next_if(|(_, c)| f(*c)) {
                to = i;
            }

            Some(&input[from..=to])
        };

        while let Some(value) = take_group(char::is_numeric) {
            let value = value.parse::<u128>().context(ParseIntSnafu)?;

            let Some(unit) = take_group(char::is_alphabetic) else {
                if let Some(&(_, chr)) = chars.peek() {
                    return UnexpectedCharacterSnafu { chr }.fail();
                } else {
                    return NoUnitSnafu { value }.fail();
                }
            };

            let unit = unit.parse::<DurationUnit>().ok().context(ParseUnitSnafu {
                unit: unit.to_string(),
            })?;

            // Check that the unit is smaller than the previous one, and that
            // it wasn't specified multiple times
            if let Some(last_unit) = last_unit {
                match unit.cmp(&last_unit) {
                    Ordering::Less => {
                        return InvalidUnitOrderingSnafu {
                            previous: last_unit,
                            current: unit,
                        }
                        .fail()
                    }
                    Ordering::Equal => return DuplicateUnitSnafu { unit }.fail(),
                    _ => (),
                }
            }

            // This try_into is needed, as Duration::from_millis was stabilized
            // in 1.3.0 but u128 was only added in 1.26.0. See
            // - https://users.rust-lang.org/t/why-duration-as-from-millis-uses-different-primitives/89302
            // - https://github.com/rust-lang/rust/issues/58580
            duration +=
                std::time::Duration::from_millis((value * unit.millis()).try_into().unwrap());
            last_unit = Some(unit);
        }

        // Buffer must not contain any remaining data
        if let Some(&(_, chr)) = chars.peek() {
            return UnexpectedCharacterSnafu { chr }.fail();
        }

        Ok(Self(duration))
    }
}

impl Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // If the inner Duration is zero, print out '0ms' as milliseconds
        // is the base unit for our Duration.
        if self.0.is_zero() {
            return write!(f, "0{}", DurationUnit::Seconds);
        }

        let mut millis = self.0.as_millis();

        for unit in DurationUnit::iter() {
            let whole = millis / unit.millis();
            let rest = millis % unit.millis();

            if whole > 0 {
                write!(f, "{}{}", whole, unit)?;
            }

            millis = rest;
        }

        Ok(())
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

impl Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Duration {
    fn add_assign(&mut self, rhs: Self) {
        self.0.add_assign(rhs.0)
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<u32> for Duration {
    type Output = Self;

    fn mul(self, rhs: u32) -> Duration {
        Self(self.0 * rhs)
    }
}

impl Div<u32> for Duration {
    type Output = Self;

    fn div(self, rhs: u32) -> Duration {
        Self(self.0 / rhs)
    }
}

impl SubAssign for Duration {
    fn sub_assign(&mut self, rhs: Self) {
        self.0.sub_assign(rhs.0)
    }
}

impl Duration {
    /// Creates a new [`Duration`] from the specified number of whole seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(std::time::Duration::from_secs(secs))
    }
}

/// Defines supported [`DurationUnit`]s. Each fragment consists of a numeric
/// value followed by a [`DurationUnit`]. The order of variants **MATTERS**.
/// It is the basis for the correct transformation of the
/// [`std::time::Duration`] back to a human-readable format, which is defined
/// in the [`Display`] implementation of [`Duration`].
#[derive(
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::EnumString,
    strum::Display,
    strum::AsRefStr,
    strum::EnumIter,
)]
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
    fn millis(&self) -> u128 {
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

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    #[rstest]
    #[case("15d2m2s1000ms", 1296123)]
    #[case("15d2m2s600ms", 1296122)]
    #[case("15d2m2s", 1296122)]
    #[case("70m", 4200)]
    #[case("1h", 3600)]
    #[case("1m", 60)]
    #[case("1s", 1)]
    fn parse_as_secs(#[case] input: &str, #[case] output: u64) {
        let dur: Duration = input.parse().unwrap();
        assert_eq!(dur.as_secs(), output);
    }

    #[rstest]
    #[case("1D", DurationParseError::ParseUnitError{unit: "D".into()})]
    #[case("2d2", DurationParseError::NoUnit{value: 2})]
    #[case("1ä", DurationParseError::InvalidInput)]
    #[case(" ", DurationParseError::InvalidInput)]
    fn parse_invalid(#[case] input: &str, #[case] expected_err: DurationParseError) {
        let err = Duration::from_str(input).unwrap_err();
        assert_eq!(err, expected_err)
    }

    #[rstest]
    #[case("15d2h1d", DurationParseError::InvalidUnitOrdering { previous: DurationUnit::Hours, current: DurationUnit::Days })]
    #[case("15d2d", DurationParseError::DuplicateUnit { unit: DurationUnit::Days })]
    fn invalid_order_or_duplicate_unit(
        #[case] input: &str,
        #[case] expected_err: DurationParseError,
    ) {
        let err = Duration::from_str(input).unwrap_err();
        assert_eq!(err, expected_err)
    }

    #[rstest]
    #[case("70m", Some("1h10m"))]
    #[case("15d2m2s", None)]
    #[case("1h20m", None)]
    #[case("1m", None)]
    #[case("1s", None)]
    fn to_string(#[case] input: &str, #[case] expected: Option<&str>) {
        let dur: Duration = input.parse().unwrap();
        match expected {
            Some(e) => assert_eq!(dur.to_string(), e),
            None => assert_eq!(dur.to_string(), input),
        }
    }

    #[test]
    fn deserialize() {
        #[derive(Deserialize)]
        struct S {
            dur: Duration,
        }

        let s: S = serde_yaml::from_str("dur: 15d2m2s").unwrap();
        assert_eq!(s.dur.as_secs(), 1296122);
    }

    #[test]
    fn serialize() {
        #[derive(Serialize)]
        struct S {
            dur: Duration,
        }

        let s = S {
            dur: "15d2m2s".parse().unwrap(),
        };
        assert_eq!(serde_yaml::to_string(&s).unwrap(), "dur: 15d2m2s\n");
    }

    #[test]
    fn add_ops() {
        let mut dur1 = Duration::from_str("20s").unwrap();
        let dur2 = Duration::from_secs(10);

        let dur = dur1 + dur2;
        assert_eq!(dur.as_secs(), 30);

        dur1 += dur2;
        assert_eq!(dur1.as_secs(), 30);
    }

    #[test]
    fn sub_ops() {
        let mut dur1 = Duration::from_str("20s").unwrap();
        let dur2 = Duration::from_secs(10);

        let dur = dur1 - dur2;
        assert_eq!(dur.as_secs(), 10);

        dur1 -= dur2;
        assert_eq!(dur1.as_secs(), 10);
    }
}
