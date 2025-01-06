use std::{ops::Deref, str::FromStr};

use k8s_openapi::apimachinery::pkg::api::resource::Quantity as K8sQuantity;

use crate::quantity::{
    macros::forward_from_impls, BinaryByteMultiple, ParseQuantityError, Quantity, Suffix,
};

pub trait JavaHeap {
    // TODO (@Techassi): Add proper error type
    /// Formats the [`MemoryQuantity`] so that it can be used as a Java heap value.
    ///
    /// This function can fail, because the [`Quantity`] has to be scaled down to at most
    fn to_java_heap_string(&self) -> Result<String, String>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryQuantity(Quantity);

impl Deref for MemoryQuantity {
    type Target = Quantity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for MemoryQuantity {
    type Err = ParseQuantityError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::from_str(input)?;
        Ok(Self(quantity))
    }
}

forward_from_impls!(Quantity, K8sQuantity, MemoryQuantity);

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
