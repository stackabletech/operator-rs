use std::{
    iter::Sum,
    ops::{Add, Deref},
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;

use crate::quantity::{macros::forward_quantity_impls, DecimalMultiple, Quantity, Suffix};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CpuQuantity(Quantity);

impl Deref for CpuQuantity {
    type Target = Quantity;

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
                suffix: Suffix::DecimalMultiple(DecimalMultiple::Empty),
            }),
            CpuQuantity::add,
        )
    }
}

forward_quantity_impls!(CpuQuantity, K8sQuantity, usize, f32, f64);

impl CpuQuantity {
    pub fn from_millis(value: u32) -> Self {
        CpuQuantity(Quantity {
            suffix: Suffix::DecimalMultiple(DecimalMultiple::Milli),
            value: value.into(),
        })
    }

    pub fn scale_to(self, suffix: Suffix) -> Self {
        Self(self.0.scale_to(suffix))
    }
}
