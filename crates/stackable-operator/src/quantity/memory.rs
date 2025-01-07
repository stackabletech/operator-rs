use std::{
    ops::{Deref, DerefMut},
    str::FromStr,
};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;

use crate::quantity::{
    macros::{forward_from_impls, forward_op_impls},
    BinaryByteMultiple, ParseQuantityError, Quantity, Suffix,
};

pub trait JavaHeap {
    // TODO (@Techassi): Add proper error type
    /// Formats the [`MemoryQuantity`] so that it can be used as a Java heap value.
    ///
    /// This function can fail, because the [`Quantity`] has to be scaled down to at most
    fn to_java_heap_string(&self) -> Result<String, String>;
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct MemoryQuantity(Quantity);

impl Deref for MemoryQuantity {
    type Target = Quantity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MemoryQuantity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromStr for MemoryQuantity {
    type Err = ParseQuantityError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::from_str(input)?;
        Ok(Self(quantity))
    }
}

impl From<MemoryQuantity> for K8sQuantity {
    fn from(value: MemoryQuantity) -> Self {
        K8sQuantity(value.to_string())
    }
}

forward_from_impls!(Quantity, K8sQuantity, MemoryQuantity);
forward_op_impls!(
    MemoryQuantity(Quantity {
        value: 0.0,
        // TODO (@Techassi): This needs to be talked about. The previous implementation used Kibi
        // here. Code which relies on that fact (for later scaling) will thus break.
        suffix: None,
    }),
    MemoryQuantity,
    usize,
    f32,
    f64
);

impl MemoryQuantity {
    pub const fn from_gibi(value: f64) -> Self {
        MemoryQuantity(Quantity {
            suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Gibi)),
            value,
        })
    }

    pub const fn from_mebi(value: f64) -> Self {
        MemoryQuantity(Quantity {
            suffix: Some(Suffix::BinaryByteMultiple(BinaryByteMultiple::Mebi)),
            value,
        })
    }
}
