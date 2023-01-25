//! Utilities for converting Kubernetes quantities to Java heap settings.
//! Since Java heap sizes are a subset of Kubernetes quantities, the conversion
//! might lose precision or fail completely.
//! In addition:
//! - decimal quantities are not supported ("2G" is invalid)
//! - units are case sensitive ("2gi" is invalid)
//! - exponential notation is not supported.
//!
//! For details on Kubernetes quantities see: <https://github.com/kubernetes/apimachinery/blob/master/pkg/api/resource/quantity.go>

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

use crate::error::{Error, OperatorResult};
use std::{
    ops::{Add, Div, Mul, Sub},
    str::FromStr,
};

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
            BinaryMultiple::Kibi => "k".to_string(),
            BinaryMultiple::Mebi => "m".to_string(),
            BinaryMultiple::Gibi => "g".to_string(),
            BinaryMultiple::Tebi => "t".to_string(),
            BinaryMultiple::Pebi => "p".to_string(),
            BinaryMultiple::Exbi => "e".to_string(),
        }
    }

    /// The exponential scale factor used when converting a `BinaryMultiple`
    /// to another one.
    fn exponential_scale_factor(&self) -> i32 {
        match self {
            BinaryMultiple::Kibi => 1,
            BinaryMultiple::Mebi => 2,
            BinaryMultiple::Gibi => 3,
            BinaryMultiple::Tebi => 4,
            BinaryMultiple::Pebi => 5,
            BinaryMultiple::Exbi => 6,
        }
    }

    pub fn get_smallest() -> Self {
        Self::Kibi
    }
}

impl FromStr for BinaryMultiple {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<BinaryMultiple> {
        match q {
            "Ki" => Ok(BinaryMultiple::Kibi),
            "Mi" => Ok(BinaryMultiple::Mebi),
            "Gi" => Ok(BinaryMultiple::Gibi),
            "Ti" => Ok(BinaryMultiple::Tebi),
            "Pi" => Ok(BinaryMultiple::Pebi),
            "Ei" => Ok(BinaryMultiple::Exbi),
            _ => Err(Error::InvalidQuantityUnit {
                value: q.to_string(),
            }),
        }
    }
}

/// Parsed representation of a K8s memory/storage resource limit.
#[derive(Clone, Copy, Debug)]
pub struct MemoryQuantity {
    pub value: f32,
    pub unit: BinaryMultiple,
}

/// Convert a (memory) [`Quantity`] to Java heap settings.
/// Quantities are usually passed on to container resources while Java heap
/// sizes need to be scaled accordingly.
/// This implements a very simple heuristic to ensure that:
/// - the quantity unit has been mapped to a java supported heap unit. Java only
///   supports up to Gibibytes while K8S quantities can be expressed in Exbibytes.
/// - the heap size has a non-zero value.
/// Fails if it can't enforce the above restrictions.
pub fn to_java_heap(q: &Quantity, factor: f32) -> OperatorResult<String> {
    let scaled = (q.0.parse::<MemoryQuantity>()? * factor).scale_for_java();
    if scaled.value < 1.0 {
        Err(Error::CannotConvertToJavaHeap {
            value: q.0.to_owned(),
        })
    } else {
        Ok(format!(
            "-Xmx{:.0}{}",
            scaled.value,
            scaled.unit.to_java_memory_unit()
        ))
    }
}

/// Convert a (memory) [`Quantity`] to a raw Java heap value of the desired `target_unit`.
/// Quantities are usually passed on to container resources while Java heap
/// sizes need to be scaled accordingly.
/// The raw heap value is converted to the specified `target_unit` (this conversion
/// is done even if specified a unit greater that Gibibytes. It is not recommended to scale
/// to anything bigger than Gibibytes.
/// This implements a very simple heuristic to ensure that:
/// - the quantity unit has been mapped to a java supported heap unit. Java only
///   supports up to Gibibytes while K8S quantities can be expressed in Exbibytes.
/// - the heap size has a non-zero value.
/// Fails if it can't enforce the above restrictions.
pub fn to_java_heap_value(
    q: &Quantity,
    factor: f32,
    target_unit: BinaryMultiple,
) -> OperatorResult<u32> {
    let scaled = (q.0.parse::<MemoryQuantity>()? * factor)
        .scale_for_java()
        .scale_to(target_unit);

    if scaled.value < 1.0 {
        Err(Error::CannotConvertToJavaHeapValue {
            value: q.0.to_owned(),
            target_unit: format!("{:?}", target_unit),
        })
    } else {
        Ok(scaled.value as u32)
    }
}

impl MemoryQuantity {
    /// Scales the unit to a value supported by Java and may even scale
    /// further down, in an attempt to avoid having zero sizes or losing too
    /// much precision.
    fn scale_for_java(&self) -> Self {
        let (norm_value, norm_unit) = match self.unit {
            BinaryMultiple::Kibi => (self.value, self.unit),
            BinaryMultiple::Mebi => (self.value, self.unit),
            BinaryMultiple::Gibi => (self.value, self.unit),
            BinaryMultiple::Tebi => (self.value * 1024.0, BinaryMultiple::Gibi),
            BinaryMultiple::Pebi => (self.value * 1024.0 * 1024.0, BinaryMultiple::Gibi),
            BinaryMultiple::Exbi => (self.value * 1024.0 * 1024.0 * 1024.0, BinaryMultiple::Gibi),
        };

        const EPS: f32 = 0.2;
        let (scaled_value, scaled_unit) = if norm_value < 1.0 || norm_value.fract() > EPS {
            match norm_unit {
                BinaryMultiple::Mebi => (norm_value * 1024.0, BinaryMultiple::Kibi),
                BinaryMultiple::Gibi => (norm_value * 1024.0, BinaryMultiple::Mebi),
                _ => (norm_value, norm_unit),
            }
        } else {
            (norm_value, norm_unit)
        };

        MemoryQuantity {
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

        MemoryQuantity {
            value: self.value * 1024f32.powi(exponent_diff),
            unit: binary_multiple,
        }
    }
}

impl Mul<f32> for MemoryQuantity {
    type Output = MemoryQuantity;

    fn mul(self, factor: f32) -> Self {
        MemoryQuantity {
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

impl Sub<MemoryQuantity> for MemoryQuantity {
    type Output = MemoryQuantity;

    fn sub(self, rhs: MemoryQuantity) -> Self::Output {
        if rhs.unit == self.unit {
            MemoryQuantity {
                value: self.value - rhs.value,
                unit: self.unit,
            }
        } else if rhs.unit < self.unit {
            MemoryQuantity {
                value: self.scale_to(rhs.unit).value - rhs.value,
                unit: rhs.unit,
            }
        } else {
            MemoryQuantity {
                value: self.value - rhs.scale_to(self.unit).value,
                unit: self.unit,
            }
        }
    }
}

impl Add<MemoryQuantity> for MemoryQuantity {
    type Output = MemoryQuantity;

    fn add(self, rhs: MemoryQuantity) -> Self::Output {
        if rhs.unit == self.unit {
            MemoryQuantity {
                value: self.value + rhs.value,
                unit: self.unit,
            }
        } else if rhs.unit < self.unit {
            MemoryQuantity {
                value: self.scale_to(rhs.unit).value + rhs.value,
                unit: rhs.unit,
            }
        } else {
            MemoryQuantity {
                value: self.value + rhs.scale_to(self.unit).value,
                unit: self.unit,
            }
        }
    }
}

impl PartialOrd for MemoryQuantity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let this_val = self.scale_to(BinaryMultiple::get_smallest()).value;
        let other_val = other.scale_to(BinaryMultiple::get_smallest()).value;
        this_val.partial_cmp(&other_val)
    }
}

impl Ord for MemoryQuantity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let this_val = self.scale_to(BinaryMultiple::get_smallest()).value;
        let other_val = other.scale_to(BinaryMultiple::get_smallest()).value;
        // Note: We just assume that our values are always not NaN, so we are actually Ord.
        // A MemoryQuantity with NaN is not permissible.
        this_val.partial_cmp(&other_val).unwrap()
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

impl FromStr for MemoryQuantity {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<Self> {
        let start_of_unit =
            q.find(|c: char| c != '.' && !c.is_numeric())
                .ok_or(Error::NoQuantityUnit {
                    value: q.to_owned(),
                })?;
        let (value, unit) = q.split_at(start_of_unit);
        Ok(MemoryQuantity {
            value: value.parse::<f32>().map_err(|_| Error::InvalidQuantity {
                value: q.to_owned(),
            })?,
            unit: unit.parse()?,
        })
    }
}

impl TryFrom<Quantity> for MemoryQuantity {
    type Error = Error;

    fn try_from(quantity: Quantity) -> OperatorResult<Self> {
        Self::try_from(&quantity)
    }
}
impl TryFrom<&Quantity> for MemoryQuantity {
    type Error = Error;

    fn try_from(quantity: &Quantity) -> OperatorResult<Self> {
        quantity.0.parse()
    }
}

#[cfg(test)]
mod test {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("256Ki", MemoryQuantity { value: 256f32, unit: BinaryMultiple::Kibi })]
    #[case("8Mi", MemoryQuantity { value: 8f32, unit: BinaryMultiple::Mebi })]
    #[case("1.5Gi", MemoryQuantity { value: 1.5f32, unit: BinaryMultiple::Gibi })]
    #[case("0.8Ti", MemoryQuantity { value: 0.8f32, unit: BinaryMultiple::Tebi })]
    #[case("3.2Pi", MemoryQuantity { value: 3.2f32, unit: BinaryMultiple::Pebi })]
    #[case("0.2Ei", MemoryQuantity { value: 0.2f32, unit: BinaryMultiple::Exbi })]
    fn test_memory_parse(#[case] input: &str, #[case] output: MemoryQuantity) {
        let got = input.parse::<MemoryQuantity>().unwrap();
        assert_eq!(got, output);
    }

    #[rstest]
    #[case("256Ki", 1.0, "-Xmx256k")]
    #[case("256Ki", 0.8, "-Xmx205k")]
    #[case("2Mi", 0.8, "-Xmx1638k")]
    #[case("1.5Gi", 0.8, "-Xmx1229m")]
    #[case("2Gi", 0.8, "-Xmx1638m")]
    pub fn test_to_java_heap(#[case] q: &str, #[case] factor: f32, #[case] heap: &str) {
        assert_eq!(heap, to_java_heap(&Quantity(q.to_owned()), factor).unwrap());
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
    pub fn test_scale_to(
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
    #[case("2Ti", 0.8, BinaryMultiple::Mebi, 1677721)]
    #[case("2Ti", 0.8, BinaryMultiple::Gibi, 1638)]
    #[case("2Ti", 1.0, BinaryMultiple::Gibi, 2048)]
    #[case("2048Ki", 1.0, BinaryMultiple::Mebi, 2)]
    #[case("2000Ki", 1.0, BinaryMultiple::Mebi, 1)]
    #[case("4000Mi", 1.0, BinaryMultiple::Gibi, 3)]
    #[case("4000Mi", 0.8, BinaryMultiple::Gibi, 3)]
    pub fn test_to_java_heap_value(
        #[case] q: &str,
        #[case] factor: f32,
        #[case] target_unit: BinaryMultiple,
        #[case] heap: u32,
    ) {
        assert_eq!(
            to_java_heap_value(&Quantity(q.to_owned()), factor, target_unit).unwrap(),
            heap
        );
    }

    #[rstest]
    #[case("1000Ki", 0.8, BinaryMultiple::Gibi)]
    #[case("1000Ki", 0.8, BinaryMultiple::Mebi)]
    #[case("1000Mi", 0.8, BinaryMultiple::Gibi)]
    #[case("1000Mi", 1.0, BinaryMultiple::Gibi)]
    #[case("1023Mi", 1.0, BinaryMultiple::Gibi)]
    #[case("1024Mi", 0.8, BinaryMultiple::Gibi)]
    pub fn test_to_java_heap_value_failure(
        #[case] q: &str,
        #[case] factor: f32,
        #[case] target_unit: BinaryMultiple,
    ) {
        assert!(to_java_heap_value(&Quantity(q.to_owned()), factor, target_unit).is_err());
    }

    #[rstest]
    #[case("1000Ki", "500Ki", "500Ki")]
    #[case("1Mi", "512Ki", "512Ki")]
    #[case("2Mi", "512Ki", "1536Ki")]
    #[case("2048Ki", "1Mi", "1024Ki")]
    pub fn test_subtraction(#[case] lhs: &str, #[case] rhs: &str, #[case] res: &str) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        let expected = MemoryQuantity::try_from(Quantity(res.to_owned())).unwrap();
        let actual = lhs - rhs;
        assert_eq!(expected, actual)
    }

    #[rstest]
    #[case("1000Ki", "500Ki", "1500Ki")]
    #[case("1Mi", "512Ki", "1536Ki")]
    #[case("2Mi", "512Ki", "2560Ki")]
    #[case("2048Ki", "1Mi", "3072Ki")]
    pub fn test_addition(#[case] lhs: &str, #[case] rhs: &str, #[case] res: &str) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        let expected = MemoryQuantity::try_from(Quantity(res.to_owned())).unwrap();
        let actual = lhs + rhs;
        assert_eq!(expected, actual)
    }

    #[rstest]
    #[case("100Ki", "100Ki", false)]
    #[case("100Ki", "100Ki", false)]
    #[case("100Ki", "100Ki", false)]
    #[case("101Ki", "100Ki", true)]
    #[case("100Ki", "101Ki", false)]
    #[case("1Mi", "100Ki", true)]
    #[case("2000Ki", "1Mi", true)]
    pub fn test_comparison(#[case] lhs: &str, #[case] rhs: &str, #[case] res: bool) {
        let lhs = MemoryQuantity::try_from(Quantity(lhs.to_owned())).unwrap();
        let rhs = MemoryQuantity::try_from(Quantity(rhs.to_owned())).unwrap();
        assert_eq!(lhs > rhs, res)
    }
}
