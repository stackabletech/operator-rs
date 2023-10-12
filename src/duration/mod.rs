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
    #[snafu(display("empty input"))]
    EmptyInput,

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

    #[snafu(display("duration overflow occurred while parsing {value}{unit} in {input}"))]
    Overflow {
        unit: DurationUnit,
        input: String,
        value: u128,
    },
}

/// A common [`Duration`] struct which is able to parse human-readable duration
/// formats, like `5s`, `24h`, `2y2h20m42s` or`15d2m2s`. It additionally
/// implements many required traits (for CRD deserialization and serialization).
#[derive(Clone, Copy, Debug, Derivative, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration(std::time::Duration);

impl FromStr for Duration {
    type Err = DurationParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        use duration_parse_error::*;
        if input.is_empty() {
            return EmptyInputSnafu.fail();
        }

        let mut chars = input.char_indices().peekable();
        let mut duration = std::time::Duration::ZERO;
        let mut last_unit = None;

        let mut take_group = |f: fn(char) -> bool| {
            let &(from, _) = chars.peek()?;
            let mut to = from;
            let mut last_char = None;

            while let Some((i, c)) = chars.next_if(|(_, c)| f(*c)) {
                to = i;
                last_char = Some(c);
            }

            // If last_char == None then we read 0 characters => fail
            Some(&input[from..(to + last_char?.len_utf8())])
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

            // First, make sure we can multiply the supplied fragment value by
            // the appropriate number of milliseconds for this unit
            let fragment_value =
                value
                    .checked_mul(unit.millis() as u128)
                    .context(OverflowSnafu {
                        input: input.to_string(),
                        value,
                        unit,
                    })?;

            // This try_into is needed, as Duration::from_millis was stabilized
            // in 1.3.0 but u128 was only added in 1.26.0. See
            // - https://users.rust-lang.org/t/why-duration-as-from-millis-uses-different-primitives/89302
            // - https://github.com/rust-lang/rust/issues/58580
            let fragment_duration = fragment_value.try_into().ok().context(OverflowSnafu {
                input: input.to_string(),
                value,
                unit,
            })?;

            // Now lets make sure that the Duration can fit the provided fragment
            // duration
            duration = duration
                .checked_add(std::time::Duration::from_millis(fragment_duration))
                .context(OverflowSnafu {
                    input: input.to_string(),
                    value,
                    unit,
                })?;

            last_unit = Some(unit);
        }

        // Buffer must not contain any remaining data
        if let Some(&(_, chr)) = chars.peek() {
            return UnexpectedCharacterSnafu { chr }.fail();
        }

        Ok(Self(duration))
    }
}

impl JsonSchema for Duration {
    fn schema_name() -> String {
        "Duration".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

impl Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // If the inner Duration is zero, print out '0s' instead of '0ms'.
        if self.0.is_zero() {
            return write!(f, "0{}", DurationUnit::Seconds);
        }

        let mut millis = self.0.as_millis();

        for unit in DurationUnit::iter() {
            let whole = millis / unit.millis() as u128;
            let rest = millis % unit.millis() as u128;

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

impl Add<Duration> for std::time::Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        self.add(rhs.0)
    }
}

impl AddAssign for Duration {
    fn add_assign(&mut self, rhs: Self) {
        self.0.add_assign(rhs.0)
    }
}

impl AddAssign<Duration> for std::time::Instant {
    fn add_assign(&mut self, rhs: Duration) {
        self.add_assign(rhs.0)
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<Duration> for std::time::Instant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        self.sub(rhs.0)
    }
}

impl SubAssign for Duration {
    fn sub_assign(&mut self, rhs: Self) {
        self.0.sub_assign(rhs.0)
    }
}

impl SubAssign<Duration> for std::time::Instant {
    fn sub_assign(&mut self, rhs: Duration) {
        self.add_assign(rhs.0)
    }
}

impl Mul<u32> for Duration {
    type Output = Self;

    fn mul(self, rhs: u32) -> Duration {
        Self(self.0 * rhs)
    }
}

impl Mul<Duration> for u32 {
    type Output = Duration;

    fn mul(self, rhs: Duration) -> Duration {
        rhs * self
    }
}

impl Div<u32> for Duration {
    type Output = Self;

    fn div(self, rhs: u32) -> Duration {
        Self(self.0 / rhs)
    }
}

impl Duration {
    /// Creates a new [`Duration`] from the specified number of whole milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        Self(std::time::Duration::from_millis(millis))
    }

    /// Creates a new [`Duration`] from the specified number of whole seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(std::time::Duration::from_secs(secs))
    }

    /// Creates a new [`Duration`] from the specified number of whole minutes.
    /// Panics if the minutes are bigger than `u64::MAX / 60 / 1000 = 307445734561825`,
    /// which is approx. 584,942,417,355 years.
    ///
    /// It is recommended to only use this function in `const` environments. It is, however,
    /// not recommended to use the function to construct [`Duration`]s from user provided input.
    /// Instead, use [`Duration::from_str`] to parse human-readable duration strings.
    pub const fn from_minutes_unchecked(minutes: u64) -> Self {
        let millis = match minutes.checked_mul(DurationUnit::Minutes.millis()) {
            Some(millis) => millis,
            None => panic!("overflow in Duration::from_minutes"),
        };
        Self::from_millis(millis)
    }

    /// Creates a new [`Duration`] from the specified number of whole hours.
    /// Panics if the hours are bigger than `u64::MAX / 60 / 60 / 1000 = 5124095576030`,
    /// which is approx. 584,942,417,355 years.
    ///
    /// It is recommended to only use this function in `const` environments. It is, however,
    /// not recommended to use the function to construct [`Duration`]s from user provided input.
    /// Instead, use [`Duration::from_str`] to parse human-readable duration strings.
    pub const fn from_hours_unchecked(hours: u64) -> Self {
        let millis = match hours.checked_mul(DurationUnit::Hours.millis()) {
            Some(millis) => millis,
            None => panic!("overflow in Duration::from_hours"),
        };
        Self::from_millis(millis)
    }

    /// Creates a new [`Duration`] from the specified number of whole days.
    /// Panics if the days are bigger than `u64::MAX / 24 / 60 / 60 / 1000 = 213503982334`,
    /// which is approx. 584,942,417,355 years.
    ///
    /// It is recommended to only use this function in `const` environments. It is, however,
    /// not recommended to use the function to construct [`Duration`]s from user provided input.
    /// Instead, use [`Duration::from_str`] to parse human-readable duration strings.
    pub const fn from_days_unchecked(days: u64) -> Self {
        let millis = match days.checked_mul(DurationUnit::Days.millis()) {
            Some(millis) => millis,
            None => panic!("overflow in Duration::from_days"),
        };
        Self::from_millis(millis)
    }
}

/// Defines supported [`DurationUnit`]s. Each fragment consists of a numeric
/// value followed by a [`DurationUnit`]. The order of variants **MATTERS**.
/// It is the basis for the correct transformation of the
/// [`std::time::Duration`] back to a human-readable format, which is defined
/// in the [`Display`] implementation of [`Duration`].
#[derive(
    Clone,
    Copy,
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
    const fn millis(&self) -> u64 {
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

    #[test]
    fn const_from() {
        assert_eq!(Duration::from_secs(42).as_secs(), 42);
        assert_eq!(Duration::from_minutes_unchecked(42).as_secs(), 42 * 60);
        assert_eq!(Duration::from_hours_unchecked(42).as_secs(), 42 * 60 * 60);
        assert_eq!(
            Duration::from_days_unchecked(42).as_secs(),
            42 * 24 * 60 * 60
        );
        assert_eq!(
            Duration::from_days_unchecked(999).as_secs(),
            999 * 24 * 60 * 60
        );
    }

    #[test]
    fn const_from_overflow() {
        let max_duration_ms = u64::MAX;
        let max_duration_days = max_duration_ms / 1000 / 60 / 60 / 24;

        assert_eq!(
            Duration::from_days_unchecked(max_duration_days).as_millis(),
            18446744073657600000 // Precision lost due to ms -> day conversion
        );
        let result =
            std::panic::catch_unwind(|| Duration::from_days_unchecked(max_duration_days + 1));
        assert!(result.is_err());
    }

    #[rstest]
    #[case("1s", 1)]
    #[case("1m", 60)]
    #[case("1h", 3600)]
    #[case("70m", 4200)]
    #[case("15d2m2s", 1296122)]
    #[case("15d2m2s600ms", 1296122)]
    #[case("15d2m2s1000ms", 1296123)]
    #[case("213503982334d", 18446744073657600)]
    fn parse_as_secs(#[case] input: &str, #[case] output: u64) {
        let dur: Duration = input.parse().unwrap();
        assert_eq!(dur.as_secs(), output);
    }

    #[rstest]
    #[case("1D", DurationParseError::ParseUnitError{unit: "D".into()})]
    #[case("2d2", DurationParseError::NoUnit{value: 2})]
    #[case("1ä", DurationParseError::ParseUnitError { unit: "ä".into() })]
    #[case(" ", DurationParseError::UnexpectedCharacter { chr: ' ' })]
    #[case("", DurationParseError::EmptyInput)]
    #[case("213503982335d", DurationParseError::Overflow { input: "213503982335d".to_string(), value: 213503982335_u128, unit: DurationUnit::Days })]
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
