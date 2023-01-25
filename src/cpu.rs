use std::str::FromStr;

use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

use crate::error::{Error, OperatorResult};

/// A wrapper around a Quantity, to make working with CPU quantities easier.
///
/// A CPU Quantity cannot have a precision finer than 'm' (millis), so we use that as
/// our internal representation (see: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/#meaning-of-cpu).
pub struct CpuQuantity {
    millis: usize,
}

impl CpuQuantity {
    pub fn as_cpu_count(&self) -> f32 {
        self.millis as f32 / 1000.
    }

    pub fn as_milli_cpus(&self) -> usize {
        self.millis
    }
}

impl FromStr for CpuQuantity {
    type Err = Error;

    fn from_str(q: &str) -> OperatorResult<Self> {
        let start_of_unit = q.find(|c: char| c != '.' && !c.is_numeric());
        if let Some(start_of_unit) = start_of_unit {
            let (value, unit) = q.split_at(start_of_unit);
            if unit != "m" {
                return Err(Error::UnsupportedQuantityPrecision {
                    value: q.to_owned(),
                });
            }
            let cpu_millis: usize = value.parse().map_err(|_| Error::InvalidCpuQuantity {
                value: q.to_owned(),
            })?;
            return Ok(CpuQuantity { millis: cpu_millis });
        } else {
            let cpus = q.parse::<f32>().map_err(|_| Error::InvalidCpuQuantity {
                value: q.to_owned(),
            })?;
            let millis_float = cpus * 1000.;
            if millis_float != millis_float.round() {
                return Err(Error::UnsupportedQuantityPrecision {
                    value: q.to_owned(),
                });
            }
            return Ok(CpuQuantity {
                millis: millis_float as usize,
            });
        }
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

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case("1", 1000)]
    #[case("1000m", 1000)]
    #[case("500m", 500)]
    #[case("2.5", 2500)]
    fn test_from_str(#[case] s: &str, #[case] millis: usize) {
        let result = CpuQuantity::from_str(s).unwrap();
        assert_eq!(millis, result.as_milli_cpus())
    }
}
