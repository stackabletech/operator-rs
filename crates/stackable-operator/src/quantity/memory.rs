use std::{
    iter::Sum,
    ops::{Add, Deref},
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;

use crate::quantity::{macros::forward_quantity_impls, BinaryMultiple, Quantity, Suffix};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct MemoryQuantity(Quantity);

impl Deref for MemoryQuantity {
    type Target = Quantity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<MemoryQuantity> for K8sQuantity {
    fn from(value: MemoryQuantity) -> Self {
        K8sQuantity(value.to_string())
    }
}

impl Sum for MemoryQuantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            MemoryQuantity(Quantity {
                value: 0.0,
                suffix: Suffix::BinaryMultiple(BinaryMultiple::Kibi),
            }),
            MemoryQuantity::add,
        )
    }
}

forward_quantity_impls!(MemoryQuantity, K8sQuantity, usize, f32, f64);

impl MemoryQuantity {
    pub const fn from_gibi(value: f64) -> Self {
        MemoryQuantity(Quantity {
            suffix: Suffix::BinaryMultiple(BinaryMultiple::Gibi),
            value,
        })
    }

    pub const fn from_mebi(value: f64) -> Self {
        MemoryQuantity(Quantity {
            suffix: Suffix::BinaryMultiple(BinaryMultiple::Mebi),
            value,
        })
    }

    pub fn scale_to(self, suffix: Suffix) -> Self {
        Self(self.0.scale_to(suffix))
    }

    pub fn ceil(self) -> Self {
        Self(Quantity {
            value: self.value.ceil(),
            suffix: self.suffix,
        })
    }
}

pub trait JavaHeap {
    // TODO (@Techassi): Add proper error type
    /// Formats the [`MemoryQuantity`] so that it can be used as a Java heap value.
    ///
    /// This function can fail, because the [`Quantity`] has to be scaled down to at most
    fn to_java_heap_string(&self) -> Result<String, String>;
}
