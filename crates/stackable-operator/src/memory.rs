//! Utilities for converting Kubernetes quantities to Java heap settings.
//! Since Java heap sizes are a subset of Kubernetes quantities, the conversion
//! might lose precision or fail completely. In addition:
//!
//! - decimal quantities are not supported ("2G" is invalid)
//! - units are case sensitive ("2gi" is invalid)
//! - exponential notation is not supported.
//!
//! For details on Kubernetes quantities see: <https://github.com/kubernetes/apimachinery/blob/master/pkg/api/resource/quantity.go>

use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use serde::{Deserialize, Serialize, de::Visitor};
use snafu::{OptionExt, ResultExt, Snafu};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Eq, Snafu)]
pub enum Error {
    #[snafu(display("cannot convert quantity {value:?} to Java heap"))]
    CannotConvertToJavaHeap { value: String },

    #[snafu(display(
        "cannot convert quantity {value:?} to Java heap value with unit {target_unit:?}"
    ))]
    CannotConvertToJavaHeapValue { value: String, target_unit: String },

    #[snafu(display("cannot scale down from kilobytes"))]
    CannotScaleDownMemoryUnit,

    #[snafu(display("invalid quantity {value:?}"))]
    InvalidQuantity {
        source: std::num::ParseFloatError,
        value: String,
    },

    #[snafu(display("invalid quantity unit {value:?}"))]
    InvalidQuantityUnit { value: String },

    #[snafu(display("no quantity unit provided for {value:?}"))]
    NoQuantityUnit { value: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum BinaryMultiple {
    Kibi,
    Mebi,
    Gibi,
    Tebi,
    Pebi,
    Exbi,
}

impl BinaryMultiple {
    pub fn to_java_memory_unit(&self) -> String {
        match self {
            Self::Kibi => "k".to_string(),
            Self::Mebi => "m".to_string(),
            Self::Gibi => "g".to_string(),
            Self::Tebi => "t".to_string(),
            Self::Pebi => "p".to_string(),
            Self::Exbi => "e".to_string(),
        }
    }

    /// The exponential scale factor used when converting a `BinaryMultiple`
    /// to another one.
    const fn exponential_scale_factor(self) -> i32 {
        match self {
            Self::Kibi => 1,
            Self::Mebi => 2,
            Self::Gibi => 3,
            Self::Tebi => 4,
            Self::Pebi => 5,
            Self::Exbi => 6,
        }
    }

    pub const fn get_smallest() -> Self {
        Self::Kibi
    }
}

impl FromStr for BinaryMultiple {
    type Err = Error;

    fn from_str(q: &str) -> Result<Self> {
        match q {
            "Ki" => Ok(Self::Kibi),
            "Mi" => Ok(Self::Mebi),
            "Gi" => Ok(Self::Gibi),
            "Ti" => Ok(Self::Tebi),
            "Pi" => Ok(Self::Pebi),
            "Ei" => Ok(Self::Exbi),
            _ => Err(Error::InvalidQuantityUnit {
                value: q.to_string(),
            }),
        }
    }
}

impl Display for BinaryMultiple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            Self::Kibi => "Ki",
            Self::Mebi => "Mi",
            Self::Gibi => "Gi",
            Self::Tebi => "Ti",
            Self::Pebi => "Pi",
            Self::Exbi => "Ei",
        };

        out.fmt(f)
    }
}

/// Convert a (memory) [`Quantity`] to Java heap settings.
///
/// Quantities are usually passed on to container resources while Java heap
/// sizes need to be scaled accordingly. This implements a very simple heuristic
/// to ensure that:
///
/// - the quantity unit has been mapped to a java supported heap unit. Java only
///   supports up to Gibibytes while K8S quantities can be expressed in Exbibytes.
/// - the heap size has a non-zero value.
///
/// Fails if it can't enforce the above restrictions.
#[deprecated(
    since = "0.33.0",
    note = "use \"-Xmx\" + MemoryQuantity::try_from(quantity).scale_to(unit).format_for_java()"
)]
pub fn to_java_heap(q: &Quantity, factor: f32) -> Result<String> {
    let scaled = (q.0.parse::<MemoryQuantity>()? * factor).scale_for_java();
    if scaled.value < 1.0 {
        Err(Error::CannotConvertToJavaHeap { value: q.0.clone() })
    } else {
        Ok(format!(
            "-Xmx{:.0}{}",
            scaled.value,
            scaled.unit.to_java_memory_unit()
        ))
    }
}

/// Convert a (memory) [`Quantity`] to a raw Java heap value of the desired `target_unit`.
///
/// Quantities are usually passed on to container resources while Java heap
/// sizes need to be scaled accordingly. The raw heap value is converted to the
/// specified `target_unit` (this conversion is done even if specified a unit
/// greater that Gibibytes. It is not recommended to scale to anything bigger
/// than Gibibytes. This implements a very simple heuristic to ensure that:
///
/// - the quantity unit has been mapped to a java supported heap unit. Java only
///   supports up to Gibibytes while K8S quantities can be expressed in Exbibytes.
/// - the heap size has a non-zero value.
///
/// Fails if it can't enforce the above restrictions.
#[deprecated(
    since = "0.33.0",
    note = "use (MemoryQuantity::try_from(quantity).scale_to(target_unit) * factor)"
)]
pub fn to_java_heap_value(q: &Quantity, factor: f32, target_unit: BinaryMultiple) -> Result<u32> {
    let scaled = (q.0.parse::<MemoryQuantity>()? * factor)
        .scale_for_java()
        .scale_to(target_unit);

    if scaled.value < 1.0 {
        Err(Error::CannotConvertToJavaHeapValue {
            value: q.0.clone(),
            target_unit: target_unit.to_string(),
        })
    } else {
        Ok(scaled.value as u32)
    }
}

/// Parsed representation of a K8s memory/storage resource limit.
#[derive(Clone, Copy, Debug)]
pub struct MemoryQuantity {
    pub value: f32,
    pub unit: BinaryMultiple,
}

impl MemoryQuantity {
    pub const fn from_gibi(gibi: f32) -> Self {
        Self {
            value: gibi,
            unit: BinaryMultiple::Gibi,
        }
    }

    pub const fn from_mebi(mebi: f32) -> Self {
        Self {
            value: mebi,
            unit: BinaryMultiple::Mebi,
        }
    }

    /// Scales down the unit to GB if it is TB or bigger.
    /// Leaves the quantity unchanged otherwise.
    fn scale_to_at_most_gb(self) -> Self {
        match self.unit {
            BinaryMultiple::Kibi | BinaryMultiple::Mebi | BinaryMultiple::Gibi => self,
            BinaryMultiple::Tebi | BinaryMultiple::Pebi | BinaryMultiple::Exbi => {
                self.scale_to(BinaryMultiple::Gibi)
            }
        }
    }

    /// Scale down the unit by one order of magnitude, i.e. GB to MB.
    fn scale_down_unit(self) -> Result<Self> {
        match self.unit {
            BinaryMultiple::Kibi => Err(Error::CannotScaleDownMemoryUnit),
            BinaryMultiple::Mebi => Ok(self.scale_to(BinaryMultiple::Kibi)),
            BinaryMultiple::Gibi => Ok(self.scale_to(BinaryMultiple::Mebi)),
            BinaryMultiple::Tebi => Ok(self.scale_to(BinaryMultiple::Gibi)),
            BinaryMultiple::Pebi => Ok(self.scale_to(BinaryMultiple::Tebi)),
            BinaryMultiple::Exbi => Ok(self.scale_to(BinaryMultiple::Pebi)),
        }
    }

    /// Floors the value of this MemoryQuantity.
    pub fn floor(&self) -> Self {
        Self {
            value: self.value.floor(),
            unit: self.unit,
        }
    }

    /// Ceils the value of this MemoryQuantity.
    pub fn ceil(&self) -> Self {
        Self {
            value: self.value.ceil(),
            unit: self.unit,
        }
    }

    /// If the MemoryQuantity value is smaller than 1 (starts with a zero), convert it to a smaller
    /// unit until the non fractional part of the value is not zero anymore.
    /// This can fail if the quantity is smaller than 1kB.
    fn ensure_no_zero(self) -> Result<Self> {
        if self.value < 1. {
            self.scale_down_unit()?.ensure_no_zero()
        } else {
            Ok(self)
        }
    }

    /// Ensure that the value of this MemoryQuantity is a natural number (not a float).
    /// This is done by picking smaller units until the fractional part is smaller than the tolerated
    /// rounding loss, and then rounding down.
    /// This can fail if the tolerated rounding loss is less than 1kB.
    fn ensure_integer(self, tolerated_rounding_loss: Self) -> Result<Self> {
        let fraction_memory = Self {
            value: self.value.fract(),
            unit: self.unit,
        };
        if fraction_memory < tolerated_rounding_loss {
            Ok(self.floor())
        } else {
            self.scale_down_unit()?
                .ensure_integer(tolerated_rounding_loss)
        }
    }

    /// Returns a value like '1355m' or '2g'. Always returns natural numbers with either 'k', 'm' or 'g',
    /// even if the values is multiple Terabytes or more.
    /// The original quantity may be rounded down to achieve a compact, natural number representation.
    /// This rounding may cause the quantity to shrink by up to 20MB.
    /// Useful to set memory quantities as JVM parameters.
    pub fn format_for_java(&self) -> Result<String> {
        let m = self
            .scale_to_at_most_gb() // Java Heap only supports specifying kb, mb or gb
            .ensure_no_zero()? // We don't want 0.9 or 0.2
            .ensure_integer(Self::from_mebi(20.))?; // Java only accepts integers not floats
        Ok(format!("{}{}", m.value, m.unit.to_java_memory_unit()))
    }

    /// Scales the unit to a value supported by Java and may even scale
    /// further down, in an attempt to avoid having zero sizes or losing too
    /// much precision.
    fn scale_for_java(self) -> Self {
        const EPS: f32 = 0.2;

        let (norm_value, norm_unit) = match self.unit {
            BinaryMultiple::Kibi | BinaryMultiple::Mebi | BinaryMultiple::Gibi => {
                (self.value, self.unit)
            }
            BinaryMultiple::Tebi => (self.value * 1024.0, BinaryMultiple::Gibi),
            BinaryMultiple::Pebi => (self.value * 1024.0 * 1024.0, BinaryMultiple::Gibi),
            BinaryMultiple::Exbi => (self.value * 1024.0 * 1024.0 * 1024.0, BinaryMultiple::Gibi),
        };

        let (scaled_value, scaled_unit) = if norm_value < 1.0 || norm_value.fract() > EPS {
            match norm_unit {
                BinaryMultiple::Mebi => (norm_value * 1024.0, BinaryMultiple::Kibi),
                BinaryMultiple::Gibi => (norm_value * 1024.0, BinaryMultiple::Mebi),
                _ => (norm_value, norm_unit),
            }
        } else {
            (norm_value, norm_unit)
        };

        Self {
            value: scaled_value,
            unit: scaled_unit,
        }
    }

    /// Scale up or down to the desired `BinaryMultiple`. Returns a new `Memory` and does
    /// not change itself.
    pub fn scale_to(&self, binary_multiple: BinaryMultiple) -> Self {
        let from_exponent: i32 = self.unit.exponential_scale_factor();
        let to_exponent: i32 = binary_multiple.exponential_scale_factor();

        let exponent_diff = from_exponent - to_exponent;

        Self {
            value: self.value * 1024f32.powi(exponent_diff),
            unit: binary_multiple,
        }
    }
}

impl Serialize for MemoryQuantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MemoryQuantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MemoryQuantityVisitor;

        impl Visitor<'_> for MemoryQuantityVisitor {
            type Value = MemoryQuantity;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid memory quantity")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                MemoryQuantity::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(MemoryQuantityVisitor)
    }
}

impl FromStr for MemoryQuantity {
    type Err = Error;

    fn from_str(q: &str) -> Result<Self> {
        let start_of_unit =
            q.find(|c: char| c != '.' && !c.is_numeric())
                .context(NoQuantityUnitSnafu {
                    value: q.to_owned(),
                })?;
        let (value, unit) = q.split_at(start_of_unit);
        Ok(Self {
            value: value.parse::<f32>().context(InvalidQuantitySnafu {
                value: q.to_owned(),
            })?,
            unit: unit.parse()?,
        })
    }
}

impl Display for MemoryQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

impl Mul<f32> for MemoryQuantity {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self {
            value: self.value * factor,
            unit: self.unit,
        }
    }
}

impl Div<f32> for MemoryQuantity {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        self * (1. / rhs)
    }
}

impl Div<Self> for MemoryQuantity {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        let rhs = rhs.scale_to(self.unit);
        self.value / rhs.value
    }
}

impl Sub<Self> for MemoryQuantity {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value - rhs.scale_to(self.unit).value,
            unit: self.unit,
        }
    }
}

impl SubAssign<Self> for MemoryQuantity {
    fn sub_assign(&mut self, rhs: Self) {
        let rhs = rhs.scale_to(self.unit);
        self.value -= rhs.value;
    }
}

impl Add<Self> for MemoryQuantity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value + rhs.scale_to(self.unit).value,
            unit: self.unit,
        }
    }
}

impl Sum for MemoryQuantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            Self {
                value: 0.0,
                unit: BinaryMultiple::Kibi,
            },
            Self::add,
        )
    }
}

impl AddAssign<Self> for MemoryQuantity {
    fn add_assign(&mut self, rhs: Self) {
        let rhs = rhs.scale_to(self.unit);
        self.value += rhs.value;
    }
}

impl PartialOrd for MemoryQuantity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let this_val = self.scale_to(BinaryMultiple::get_smallest()).value;
        let other_val = other.scale_to(BinaryMultiple::get_smallest()).value;
        this_val.partial_cmp(&other_val)
    }
}

impl PartialEq for MemoryQuantity {
    fn eq(&self, other: &Self) -> bool {
        let this_val = self.scale_to(BinaryMultiple::get_smallest()).value;
        let other_val = other.scale_to(BinaryMultiple::get_smallest()).value;
        this_val == other_val
    }
}

impl Eq for MemoryQuantity {}

impl TryFrom<Quantity> for MemoryQuantity {
    type Error = Error;

    fn try_from(quantity: Quantity) -> Result<Self> {
        Self::try_from(&quantity)
    }
}

impl TryFrom<&Quantity> for MemoryQuantity {
    type Error = Error;

    fn try_from(quantity: &Quantity) -> Result<Self> {
        quantity.0.parse()
    }
}

impl From<MemoryQuantity> for Quantity {
    fn from(quantity: MemoryQuantity) -> Self {
        Self::from(&quantity)
    }
}

impl From<&MemoryQuantity> for Quantity {
    fn from(quantity: &MemoryQuantity) -> Self {
        Self(format!("{quantity}"))
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("256Ki", MemoryQuantity { value: 256.0, unit: BinaryMultiple::Kibi })]
    #[case("49041204Ki", MemoryQuantity { value: 49_041_204.0, unit: BinaryMultiple::Kibi })]
    #[case("8Mi", MemoryQuantity { value: 8.0, unit: BinaryMultiple::Mebi })]
    #[case("1.5Gi", MemoryQuantity { value: 1.5, unit: BinaryMultiple::Gibi })]
    #[case("0.8Ti", MemoryQuantity { value: 0.8, unit: BinaryMultiple::Tebi })]
    #[case("3.2Pi", MemoryQuantity { value: 3.2, unit: BinaryMultiple::Pebi })]
    #[case("0.2Ei", MemoryQuantity { value: 0.2, unit: BinaryMultiple::Exbi })]
    fn from_str_pass(#[case] input: &str, #[case] expected: MemoryQuantity) {
        let got = MemoryQuantity::from_str(input).unwrap();
        assert_eq!(got, expected);
    }

    #[rstest]
    #[case("256Ki")]
    #[case("1.6Mi")]
    #[case("1.2Gi")]
    #[case("1.6Gi")]
    #[case("1Gi")]
    fn try_from_quantity(#[case] input: String) {
        let quantity = MemoryQuantity::try_from(Quantity(input.clone())).unwrap();
        let actual = format!("{quantity}");
        assert_eq!(input, actual);
    }

    #[rstest]
    #[case("256Ki", "256k")]
    #[case("1.6Mi", "1m")]
    #[case("1.2Gi", "1228m")]
    #[case("1.6Gi", "1638m")]
    #[case("1Gi", "1g")]
    fn format_java(#[case] input: String, #[case] expected: String) {
        let m = MemoryQuantity::try_from(Quantity(input)).unwrap();
        let actual = m.format_for_java().unwrap();
        assert_eq!(expected, actual);
    }

    #[rstest]
    #[case(2000f32, BinaryMultiple::Kibi, BinaryMultiple::Kibi, 2000f32)]
    #[case(2000f32, BinaryMultiple::Kibi, BinaryMultiple::Mebi, 2000f32/1024f32)]
    #[case(2000f32, BinaryMultiple::Kibi, BinaryMultiple::Gibi, 2000f32/1024f32/1024f32)]
    #[case(2000f32, BinaryMultiple::Kibi, BinaryMultiple::Tebi, 2000f32/1024f32/1024f32/1024f32)]
    #[case(2000f32, BinaryMultiple::Kibi, BinaryMultiple::Pebi, 2000f32/1024f32/1024f32/1024f32/1024f32)]
    #[case(2000f32, BinaryMultiple::Pebi, BinaryMultiple::Mebi, 2000f32*1024f32*1024f32*1024f32)]
    #[case(2000f32, BinaryMultiple::Pebi, BinaryMultiple::Kibi, 2000f32*1024f32*1024f32*1024f32*1024f32)]
    #[case(2000f32, BinaryMultiple::Exbi, BinaryMultiple::Pebi, 2000f32*1024f32)]
    fn scale_to(
        #[case] value: f32,
        #[case] unit: BinaryMultiple,
        #[case] target_unit: BinaryMultiple,
        #[case] expected: f32,
    ) {
        let memory = MemoryQuantity { value, unit };
        let scaled_memory = memory.scale_to(target_unit);
        assert_eq!(scaled_memory.value, expected);
    }

    #[rstest]
    #[case("256Ki", 1.0, BinaryMultiple::Kibi, 256)]
    #[case("256Ki", 0.8, BinaryMultiple::Kibi, 204)]
    #[case("2Mi", 0.8, BinaryMultiple::Kibi, 1638)]
    #[case("1.5Gi", 0.8, BinaryMultiple::Mebi, 1228)]
    #[case("2Gi", 0.8, BinaryMultiple::Mebi, 1638)]
    #[case("2Ti", 0.8, BinaryMultiple::Mebi, 1_677_721)]
    #[case("2Ti", 0.8, BinaryMultiple::Gibi, 1638)]
    #[case("2Ti", 1.0, BinaryMultiple::Gibi, 2048)]
    #[case("2048Ki", 1.0, BinaryMultiple::Mebi, 2)]
    #[case("2000Ki", 1.0, BinaryMultiple::Mebi, 1)]
    #[case("4000Mi", 1.0, BinaryMultiple::Gibi, 3)]
    #[case("4000Mi", 0.8, BinaryMultiple::Gibi, 3)]
    fn to_java_heap_value_pass(
        #[case] q: &str,
        #[case] factor: f32,
        #[case] target_unit: BinaryMultiple,
        #[case] heap: u32,
    ) {
        #[allow(deprecated)] // allow use of the deprecated 'to_java_heap' function to test it
        let actual = to_java_heap_value(&Quantity(q.to_owned()), factor, target_unit).unwrap();
        assert_eq!(actual, heap);
    }

    #[rstest]
    #[case("1000Ki", 0.8, BinaryMultiple::Gibi)]
    #[case("1000Ki", 0.8, BinaryMultiple::Mebi)]
    #[case("1000Mi", 0.8, BinaryMultiple::Gibi)]
    #[case("1000Mi", 1.0, BinaryMultiple::Gibi)]
    #[case("1023Mi", 1.0, BinaryMultiple::Gibi)]
    #[case("1024Mi", 0.8, BinaryMultiple::Gibi)]
    fn to_java_heap_value_fail(
        #[case] q: &str,
        #[case] factor: f32,
        #[case] target_unit: BinaryMultiple,
    ) {
        #[allow(deprecated)] // allow use of the deprecated 'to_java_heap' function to test it
        let result = to_java_heap_value(&Quantity(q.to_owned()), factor, target_unit);
        assert!(result.is_err());
    }

    #[rstest]
    #[case("1000Ki", "500Ki", "500Ki")]
    #[case("1Mi", "512Ki", "512Ki")]
    #[case("2Mi", "512Ki", "1536Ki")]
    #[case("2048Ki", "1Mi", "1024Ki")]
    fn subtraction(#[case] lhs: &str, #[case] rhs: &str, #[case] res: &str) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        let expected = MemoryQuantity::try_from(Quantity(res.to_owned())).unwrap();
        let actual = lhs - rhs;
        assert_eq!(expected, actual);

        let mut actual = lhs;
        actual -= rhs;
        assert_eq!(expected, actual);
    }

    #[rstest]
    #[case("1000Ki", "500Ki", "1500Ki")]
    #[case("1Mi", "512Ki", "1536Ki")]
    #[case("2Mi", "512Ki", "2560Ki")]
    #[case("2048Ki", "1Mi", "3072Ki")]
    fn addition(#[case] lhs: &str, #[case] rhs: &str, #[case] res: &str) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        let expected = MemoryQuantity::try_from(Quantity(res.to_owned())).unwrap();
        let actual = lhs + rhs;
        assert_eq!(expected, actual);

        let mut actual = MemoryQuantity::from_mebi(0.0);
        actual += lhs;
        actual += rhs;
        assert_eq!(expected, actual);
    }

    #[rstest]
    #[case("100Ki", "100Ki", false)]
    #[case("100Ki", "100Ki", false)]
    #[case("100Ki", "100Ki", false)]
    #[case("101Ki", "100Ki", true)]
    #[case("100Ki", "101Ki", false)]
    #[case("1Mi", "100Ki", true)]
    #[case("2000Ki", "1Mi", true)]
    fn partial_ord(#[case] lhs: &str, #[case] rhs: &str, #[case] res: bool) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        assert_eq!(lhs > rhs, res);
    }

    #[rstest]
    #[case("100Ki", "100Ki", true)]
    #[case("100Ki", "200Ki", false)]
    #[case("1Mi", "1024Ki", true)]
    #[case("1024Ki", "1Mi", true)]
    fn partial_eq(#[case] lhs: &str, #[case] rhs: &str, #[case] res: bool) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        assert_eq!(lhs == rhs, res);
    }

    #[rstest]
    #[case(MemoryQuantity::from_mebi(1536.0), "memory: 1536Mi\n")]
    #[case(MemoryQuantity::from_mebi(100.0), "memory: 100Mi\n")]
    #[case(MemoryQuantity::from_gibi(10.0), "memory: 10Gi\n")]
    #[case(MemoryQuantity::from_gibi(1.0), "memory: 1Gi\n")]
    fn serialize(#[case] memory: MemoryQuantity, #[case] expected: &str) {
        #[derive(Serialize)]
        struct Memory {
            memory: MemoryQuantity,
        }

        let memory = Memory { memory };
        let output = serde_yaml::to_string(&memory).unwrap();

        assert_eq!(output, expected);
    }

    #[rstest]
    #[case("memory: 1536Mi", MemoryQuantity::from_mebi(1536.0))]
    #[case("memory: 100Mi", MemoryQuantity::from_mebi(100.0))]
    #[case("memory: 10Gi", MemoryQuantity::from_gibi(10.0))]
    #[case("memory: 1Gi", MemoryQuantity::from_gibi(1.0))]
    fn deserialize(#[case] input: &str, #[case] expected: MemoryQuantity) {
        #[derive(Deserialize)]
        struct Memory {
            memory: MemoryQuantity,
        }

        let memory: Memory = serde_yaml::from_str(input).unwrap();
        assert_eq!(memory.memory, expected);
    }
}
