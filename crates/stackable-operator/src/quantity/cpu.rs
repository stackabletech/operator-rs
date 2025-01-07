use std::{ops::Deref, str::FromStr};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;

use crate::quantity::{
    macros::{forward_from_impls, forward_op_impls},
    DecimalByteMultiple, ParseQuantityError, Quantity, Suffix,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CpuQuantity(Quantity);

impl Deref for CpuQuantity {
    type Target = Quantity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for CpuQuantity {
    type Err = ParseQuantityError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::from_str(input)?;
        Ok(Self(quantity))
    }
}

forward_from_impls!(Quantity, K8sQuantity, CpuQuantity);
forward_op_impls!(
    CpuQuantity(Quantity {
        value: 0.0,
        suffix: None,
    }),
    CpuQuantity,
    usize,
    f32,
    f64
);

impl CpuQuantity {
    pub fn from_millis(value: u32) -> Self {
        CpuQuantity(Quantity {
            suffix: Some(Suffix::DecimalByteMultiple(DecimalByteMultiple::Milli)),
            value: value.into(),
        })
    }
}
