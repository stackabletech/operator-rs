use std::ops::Deref;

use proc_macro2::TokenStream;
use syn::Ident;

use crate::{attrs::common::ContainerAttributes, codegen::common::ContainerVersion};

/// This trait helps to unify versioned containers, like structs and enums.
///
/// This trait is implemented by wrapper structs, which wrap the generic
/// [`VersionedContainer`] struct. The generic type parameter `D` describes the
/// kind of data, like [`DataStruct`](syn::DataStruct) in case of a struct and
/// [`DataEnum`](syn::DataEnum) in case of an enum.
/// The type parameter `I` describes the type of the versioned items, like
/// [`VersionedField`][1] and [`VersionedVariant`][2].
///
/// [1]: crate::codegen::vstruct::field::VersionedField
/// [2]: crate::codegen::venum::variant::VersionedVariant
pub(crate) trait Container<D, I>
where
    Self: Sized + Deref<Target = VersionedContainer<I>>,
{
    /// Creates a new versioned container.
    fn new(ident: Ident, data: D, attributes: ContainerAttributes) -> syn::Result<Self>;

    /// This generates the complete code for a single versioned container.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the container with the appropriate items (fields or variants)
    /// Additionally, it generates `From` implementations, which enable
    /// conversion from an older to a newer version.
    fn generate_tokens(&self) -> TokenStream;
}

#[derive(Debug)]
pub(crate) struct VersionedContainer<I> {
    pub(crate) versions: Vec<ContainerVersion>,
    pub(crate) items: Vec<I>,
    pub(crate) ident: Ident,

    pub(crate) from_ident: Ident,
    pub(crate) skip_from: bool,
}
