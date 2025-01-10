use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, Deref},
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;
use snafu::Snafu;

use crate::quantity::{
    macros::forward_quantity_impls, DecimalExponent, DecimalMultiple, Quantity, Suffix,
};

#[derive(Debug, Snafu)]
pub struct ParseSuffixError {
    input: String,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum CpuSuffix {
    DecimalMultiple(DecimalMultiple),
    DecimalExponent(DecimalExponent),
}

impl FromStr for CpuSuffix {
    type Err = ParseSuffixError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if let Ok(decimal_multiple) = DecimalMultiple::from_str(input) {
            return Ok(Self::DecimalMultiple(decimal_multiple));
        }

        if input.starts_with(['e', 'E']) {
            if let Ok(decimal_exponent) = f64::from_str(&input[1..]) {
                return Ok(Self::DecimalExponent(DecimalExponent::from(
                    decimal_exponent,
                )));
            }
        }

        ParseSuffixSnafu { input }.fail()
    }
}

impl Display for CpuSuffix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Default for CpuSuffix {
    fn default() -> Self {
        CpuSuffix::DecimalMultiple(DecimalMultiple::Empty)
    }
}

impl Suffix for CpuSuffix {
    fn factor(&self) -> f64 {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CpuQuantity(Quantity<CpuSuffix>);

impl Deref for CpuQuantity {
    type Target = Quantity<CpuSuffix>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<CpuQuantity> for K8sQuantity {
    fn from(value: CpuQuantity) -> Self {
        K8sQuantity(value.to_string())
    }
}

impl Sum for CpuQuantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            CpuQuantity(Quantity {
                value: 0.0,
                suffix: CpuSuffix::DecimalMultiple(DecimalMultiple::Empty),
            }),
            CpuQuantity::add,
        )
    }
}

forward_quantity_impls!(CpuQuantity, K8sQuantity, usize, f32, f64);

impl CpuQuantity {
    pub fn from_millis(value: u32) -> Self {
        CpuQuantity(Quantity {
            suffix: CpuSuffix::DecimalMultiple(DecimalMultiple::Milli),
            value: value.into(),
        })
    }

    pub fn scale_to(self, suffix: CpuSuffix) -> Self {
        Self(self.0.scale_to(suffix))
    }
}
