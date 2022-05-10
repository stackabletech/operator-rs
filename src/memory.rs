//! Utilities for converting Kubernetes quantities to Java heap settings.

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

use crate::error::{Error, OperatorResult};
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum BinaryMultiple {
    Kibi,
    Mebi,
    Gibi,
    Tebi,
    Pebi,
    Exbi,
}

impl BinaryMultiple {
    pub fn to_legacy(&self) -> String {
        match self {
            BinaryMultiple::Kibi => "k".to_string(),
            BinaryMultiple::Mebi => "m".to_string(),
            BinaryMultiple::Gibi => "g".to_string(),
            BinaryMultiple::Tebi => "t".to_string(),
            BinaryMultiple::Pebi => "p".to_string(),
            BinaryMultiple::Exbi => "e".to_string(),
        }
    }

    pub fn upscale(&self) -> Self {
        match self {
            BinaryMultiple::Kibi => BinaryMultiple::Kibi,
            BinaryMultiple::Mebi => BinaryMultiple::Kibi,
            BinaryMultiple::Gibi => BinaryMultiple::Mebi,
            BinaryMultiple::Tebi => BinaryMultiple::Gibi,
            BinaryMultiple::Pebi => BinaryMultiple::Tebi,
            BinaryMultiple::Exbi => BinaryMultiple::Pebi,
        }
    }
}

impl FromStr for BinaryMultiple {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<BinaryMultiple> {
        let lq = q.to_lowercase();
        match lq.as_str() {
            "ki" | "kib" => Ok(BinaryMultiple::Kibi),
            "mi" | "mib" => Ok(BinaryMultiple::Mebi),
            "gi" | "gib" => Ok(BinaryMultiple::Gibi),
            "ti" | "tib" => Ok(BinaryMultiple::Tebi),
            "pi" | "pib" => Ok(BinaryMultiple::Pebi),
            "ei" | "eib" => Ok(BinaryMultiple::Exbi),
            _ => Err(Error::InvalidQuantityUnit {
                value: q.to_string(),
            }),
        }
    }
}

/// Easily transform K8S memory resources to Java heap options.
#[derive(Clone, Debug, PartialEq)]
pub struct Memory {
    value: f32,
    unit: BinaryMultiple,
}

impl Memory {
    /// Scale by the given factor. If the factor is less then one
    /// the unit granularity is increased one level to ensure eventual
    /// conversions to Java heap settings don't end up with zero values..
    pub fn scale(&self, factor: f32) -> Self {
        if factor < 1.0 && self.unit != BinaryMultiple::Kibi {
            Memory {
                value: self.value * factor * 1024.0,
                unit: self.unit.upscale(),
            }
        } else {
            Memory {
                value: self.value * factor,
                unit: self.unit.clone(),
            }
        }
    }

    /// The Java heap settings do not support fractional values therefore
    /// this cannot be implemented without loss of precision.
    pub fn to_java_heap(&self, factor: f32) -> String {
        let scaled = self.scale(factor);
        format!("-Xmx{:.0}{}", scaled.value, scaled.unit.to_legacy())
    }
}

impl FromStr for Memory {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<Self> {
        let mut v = String::from("");
        let mut u = String::from("");

        for c in q.chars() {
            if c.is_numeric() || c == '.' {
                v.push(c);
            } else {
                u.push(c);
            }
        }
        Ok(Memory {
            value: v.parse::<f32>().map_err(|_| Error::InvalidQuantity {
                value: q.to_owned(),
            })?,
            unit: u.parse()?,
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
    #[case("256ki", Memory { value: 256f32, unit: BinaryMultiple::Kibi })]
    #[case("8Mib", Memory { value: 8f32, unit: BinaryMultiple::Mebi })]
    #[case("1.5Gi", Memory { value: 1.5f32, unit: BinaryMultiple::Gibi })]
    #[case("0.8tib", Memory { value: 0.8f32, unit: BinaryMultiple::Tebi })]
    #[case("3.2Pi", Memory { value: 3.2f32, unit: BinaryMultiple::Pebi })]
    #[case("0.2ei", Memory { value: 0.2f32, unit: BinaryMultiple::Exbi })]
    pub fn test_memory_parse(#[case] input: &str, #[case] output: Memory) {
        let got = input.parse::<Memory>().unwrap();
        assert_eq!(got, output);
    }

    #[rstest]
    #[case("256ki", 1.0, "-Xmx256k")]
    #[case("256ki", 0.8, "-Xmx205k")]
    #[case("2mib", 0.8, "-Xmx1638k")]
    #[case("1.5GiB", 0.8, "-Xmx1229m")]
    pub fn test_memory_scale(#[case] q: &str, #[case] factor: f32, #[case] heap: &str) {
        let qu: Memory = Quantity(q.to_owned()).try_into().unwrap();
        assert_eq!(heap, qu.to_java_heap(factor));
    }
}
