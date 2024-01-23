use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, AddAssign, Div, Mul, MulAssign},
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use serde::{de::Visitor, Deserialize, Serialize};

use crate::error::{Error, OperatorResult};

/// A representation of CPU quantities with milli precision.
/// Supports conversion from [`Quantity`].
///
/// A CPU quantity cannot have a precision finer than 'm' (millis) in Kubernetes.
/// So we use that as our internal representation (see:
/// `<https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/#meaning-of-cpu>`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CpuQuantity {
    millis: usize,
}

impl CpuQuantity {
    pub const fn from_millis(millis: usize) -> Self {
        Self { millis }
    }

    pub fn as_cpu_count(&self) -> f32 {
        self.millis as f32 / 1000.
    }

    pub const fn as_milli_cpus(&self) -> usize {
        self.millis
    }
}

impl Serialize for CpuQuantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for CpuQuantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CpuQuantityVisitor;

        impl<'de> Visitor<'de> for CpuQuantityVisitor {
            type Value = CpuQuantity;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid CPU quantiry")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                CpuQuantity::from_str(v).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(CpuQuantityVisitor)
    }
}

impl Display for CpuQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.millis < 1000 {
            true => write!(f, "{}m", self.millis),
            false => write!(f, "{}", self.as_cpu_count()),
        }
    }
}

impl FromStr for CpuQuantity {
    type Err = Error;

    /// Only two formats can be parsed
    /// - {usize}m
    /// - {f32}
    /// For the float, only milli-precision is supported.
    /// Using more precise values will trigger an error, and using any other
    /// unit than 'm' or None will also trigger an error.
    fn from_str(q: &str) -> OperatorResult<Self> {
        let start_of_unit = q.find(|c: char| c != '.' && !c.is_numeric());
        if let Some(start_of_unit) = start_of_unit {
            let (value, unit) = q.split_at(start_of_unit);
            if unit != "m" {
                return Err(Error::UnsupportedCpuQuantityPrecision {
                    value: q.to_owned(),
                });
            }
            let cpu_millis: usize = value.parse().map_err(|_| Error::InvalidCpuQuantity {
                value: q.to_owned(),
            })?;
            Ok(Self::from_millis(cpu_millis))
        } else {
            let cpus = q.parse::<f32>().map_err(|_| Error::InvalidCpuQuantity {
                value: q.to_owned(),
            })?;
            let millis_float = cpus * 1000.;
            if millis_float != millis_float.round() {
                return Err(Error::UnsupportedCpuQuantityPrecision {
                    value: q.to_owned(),
                });
            }
            Ok(Self::from_millis(millis_float as usize))
        }
    }
}

impl From<CpuQuantity> for Quantity {
    fn from(quantity: CpuQuantity) -> Self {
        Self::from(&quantity)
    }
}

impl From<&CpuQuantity> for Quantity {
    fn from(quantity: &CpuQuantity) -> Self {
        Quantity(format!("{}", quantity.as_cpu_count()))
    }
}

impl TryFrom<&Quantity> for CpuQuantity {
    type Error = Error;

    fn try_from(q: &Quantity) -> Result<Self, Self::Error> {
        Self::from_str(&q.0)
    }
}

impl TryFrom<Quantity> for CpuQuantity {
    type Error = Error;

    fn try_from(q: Quantity) -> Result<Self, Self::Error> {
        Self::try_from(&q)
    }
}

impl Add<CpuQuantity> for CpuQuantity {
    type Output = CpuQuantity;

    fn add(self, rhs: CpuQuantity) -> Self::Output {
        CpuQuantity::from_millis(self.millis + rhs.millis)
    }
}

impl AddAssign<CpuQuantity> for CpuQuantity {
    fn add_assign(&mut self, rhs: CpuQuantity) {
        self.millis += rhs.millis;
    }
}

impl Mul<usize> for CpuQuantity {
    type Output = CpuQuantity;

    fn mul(self, rhs: usize) -> Self::Output {
        Self {
            millis: self.millis * rhs,
        }
    }
}

impl MulAssign<usize> for CpuQuantity {
    fn mul_assign(&mut self, rhs: usize) {
        self.millis *= rhs;
    }
}

impl Mul<f32> for CpuQuantity {
    type Output = CpuQuantity;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            millis: (self.millis as f32 * rhs) as usize,
        }
    }
}

impl Div<CpuQuantity> for CpuQuantity {
    type Output = f32;

    fn div(self, rhs: CpuQuantity) -> Self::Output {
        self.millis as f32 / rhs.millis as f32
    }
}

impl MulAssign<f32> for CpuQuantity {
    fn mul_assign(&mut self, rhs: f32) {
        self.millis = (self.millis as f32 * rhs) as usize;
    }
}

impl Sum for CpuQuantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(CpuQuantity { millis: 0 }, CpuQuantity::add)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case("1", 1000)]
    #[case("1000m", 1000)]
    #[case("500m", 500)]
    #[case("2.5", 2500)]
    #[case("0.2", 200)]
    #[case("0.02", 20)]
    #[case("0.002", 2)]
    fn test_from_str(#[case] s: &str, #[case] millis: usize) {
        let result = CpuQuantity::from_str(s).unwrap();
        assert_eq!(millis, result.as_milli_cpus())
    }

    #[rstest]
    #[case("1.11111")]
    #[case("1000.1m")]
    #[case("500k")]
    #[case("0.0002")]
    fn test_from_str_err(#[case] s: &str) {
        let result = CpuQuantity::from_str(s);
        assert!(result.is_err());
    }

    #[rstest]
    #[case(CpuQuantity::from_millis(10000), "10")]
    #[case(CpuQuantity::from_millis(1500), "1.5")]
    #[case(CpuQuantity::from_millis(999), "999m")]
    #[case(CpuQuantity::from_millis(500), "500m")]
    #[case(CpuQuantity::from_millis(100), "100m")]
    #[case(CpuQuantity::from_millis(2000), "2")]
    #[case(CpuQuantity::from_millis(1000), "1")]
    fn test_display_to_string(#[case] cpu: CpuQuantity, #[case] expected: &str) {
        assert_eq!(cpu.to_string(), expected)
    }

    #[rstest]
    #[case(CpuQuantity::from_millis(10000), "cpu: '10'\n")]
    #[case(CpuQuantity::from_millis(1500), "cpu: '1.5'\n")]
    #[case(CpuQuantity::from_millis(999), "cpu: 999m\n")]
    #[case(CpuQuantity::from_millis(500), "cpu: 500m\n")]
    #[case(CpuQuantity::from_millis(100), "cpu: 100m\n")]
    #[case(CpuQuantity::from_millis(2000), "cpu: '2'\n")]
    #[case(CpuQuantity::from_millis(1000), "cpu: '1'\n")]
    fn test_serialize(#[case] cpu: CpuQuantity, #[case] expected: &str) {
        #[derive(Serialize)]
        struct Cpu {
            cpu: CpuQuantity,
        }

        let cpu = Cpu { cpu };
        let output = serde_yaml::to_string(&cpu).unwrap();

        assert_eq!(output, expected);
    }

    #[rstest]
    #[case("cpu: '10'", CpuQuantity::from_millis(10000))]
    #[case("cpu: '1.5'", CpuQuantity::from_millis(1500))]
    #[case("cpu: 999m", CpuQuantity::from_millis(999))]
    #[case("cpu: 500m", CpuQuantity::from_millis(500))]
    #[case("cpu: 100m", CpuQuantity::from_millis(100))]
    #[case("cpu: 2", CpuQuantity::from_millis(2000))]
    #[case("cpu: 1", CpuQuantity::from_millis(1000))]
    fn test_deserialize(#[case] input: &str, #[case] expected: CpuQuantity) {
        #[derive(Deserialize)]
        struct Cpu {
            cpu: CpuQuantity,
        }

        let cpu: Cpu = serde_yaml::from_str(input).unwrap();
        assert_eq!(cpu.cpu, expected);
    }
}
