use std::ops::Deref;

use proc_macro2::TokenStream;
use syn::{Attribute, Ident};

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
    fn new(
        ident: Ident,
        data: D,
        attributes: ContainerAttributes,
        original_attributes: Vec<Attribute>,
    ) -> syn::Result<Self>;

    /// This generates the complete code for a single versioned container.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the container with the appropriate items (fields or variants)
    /// Additionally, it generates `From` implementations, which enable
    /// conversion from an older to a newer version.
    fn generate_tokens(&self) -> TokenStream;
}

/// Stores individual versions of a single container.
///
/// Each version tracks item actions, which describe if the item was added,
/// renamed or deprecated in that particular version. Items which are not
/// versioned are included in every version of the container.
#[derive(Debug)]
pub(crate) struct VersionedContainer<I> {
    /// List of declared versions for this container. Each version generates a
    /// definition with appropriate items.
    pub(crate) versions: Vec<ContainerVersion>,

    /// List of items defined in the original container. How, and if, an item
    /// should generate code, is decided by the currently generated version.
    pub(crate) items: Vec<I>,

    /// The ident, or name, of the versioned container.
    pub(crate) ident: Ident,

    /// The name of the container used in `From` implementations.
    pub(crate) from_ident: Ident,

    /// Whether the [`From`] implementation generation should be skipped for all
    /// versions of this container.
    pub(crate) skip_from: bool,

    /// The original attributes that were added to the container.
    pub(crate) original_attributes: Vec<Attribute>,
}
