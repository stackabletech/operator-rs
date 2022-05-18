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
use std::{ops::Mul, str::FromStr};

// Important: Do not change the order of the elements. The enum discriminants are used
// for unit conversion and calculations!
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
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

/// Easily transform K8S memory resources to Java heap options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    value: f32,
    unit: BinaryMultiple,
}

/// Convert a (memory) [`Quantity`] to Java heap settings.
/// Quantities are usually passed on to container resources whily Java heap
/// sizes need to be scaled accordingly.
/// This implements a very simple heuristic to ensure that:
/// - the quantity unit has been mapped to a java supported heap unit. Java only
///   supports up to Gibibytes while K8S quantities can be expressed in Exbibytes.
/// - the heap size has a non-zero value.
/// Fails if it can't enforce the above restrictions.
pub fn to_java_heap(q: &Quantity, factor: f32) -> OperatorResult<String> {
    let scaled = (q.0.parse::<Memory>()? * factor).scale_for_java();
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

impl Memory {
    /// Scales the unit to a value supported by Java and may even scale
    /// further down, in an attempt to avoid having zero sizes or losing too
    /// much precision.
    pub fn scale_for_java(&self) -> Self {
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

        Memory {
            value: scaled_value,
            unit: scaled_unit,
        }
    }

    /// Scale up or down to the desired `binary_multiple`. Returns a new `Memory` and does
    /// not change itself.
    pub fn scale_to(&self, binary_multiple: BinaryMultiple) -> Self {
        let self_discriminant = self.unit as i32;
        let to_discriminant = binary_multiple as i32;

        let diff: i32 = (self_discriminant - to_discriminant) as i32;

        Memory {
            value: self.value * 1024f32.powi(diff),
            unit: binary_multiple,
        }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn unit(&self) -> BinaryMultiple {
        self.unit
    }
}

impl Mul<f32> for Memory {
    type Output = Memory;

    fn mul(self, factor: f32) -> Self {
        Memory {
            value: self.value * factor,
            unit: self.unit,
        }
    }
}

impl FromStr for Memory {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<Self> {
        let start_of_unit =
            q.find(|c: char| c != '.' && !c.is_numeric())
                .ok_or(Error::NoQuantityUnit {
                    value: q.to_owned(),
                })?;
        let (value, unit) = q.split_at(start_of_unit);
        Ok(Memory {
            value: value.parse::<f32>().map_err(|_| Error::InvalidQuantity {
                value: q.to_owned(),
            })?,
            unit: unit.parse()?,
        })
    }
}

impl TryFrom<Quantity> for Memory {
    type Error = Error;

    fn try_from(quantity: Quantity) -> OperatorResult<Self> {
        quantity.0.parse()
    }
}

#[cfg(test)]
mod test {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("256Ki", Memory { value: 256f32, unit: BinaryMultiple::Kibi })]
    #[case("8Mi", Memory { value: 8f32, unit: BinaryMultiple::Mebi })]
    #[case("1.5Gi", Memory { value: 1.5f32, unit: BinaryMultiple::Gibi })]
    #[case("0.8Ti", Memory { value: 0.8f32, unit: BinaryMultiple::Tebi })]
    #[case("3.2Pi", Memory { value: 3.2f32, unit: BinaryMultiple::Pebi })]
    #[case("0.2Ei", Memory { value: 0.2f32, unit: BinaryMultiple::Exbi })]
    pub fn test_memory_parse(#[case] input: &str, #[case] output: Memory) {
        let got = input.parse::<Memory>().unwrap();
        assert_eq!(got, output);
    }

    #[rstest]
    #[case("256Ki", 1.0, "-Xmx256k")]
    #[case("256Ki", 0.8, "-Xmx205k")]
    #[case("2Mi", 0.8, "-Xmx1638k")]
    #[case("1.5Gi", 0.8, "-Xmx1229m")]
    #[case("2Gi", 0.8, "-Xmx1638m")]
    pub fn test_memory_scale(#[case] q: &str, #[case] factor: f32, #[case] heap: &str) {
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
        #[case] desired_unit: BinaryMultiple,
        #[case] expected: f32,
    ) {
        let memory = Memory { value, unit };
        let scaled_memory = memory.scale_to(desired_unit);
        assert_eq!(scaled_memory.value, expected);
    }
}
