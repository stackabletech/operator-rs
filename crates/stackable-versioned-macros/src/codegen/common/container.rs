use std::ops::Deref;

use proc_macro2::TokenStream;
use syn::Ident;

use crate::{attrs::common::ContainerAttributes, codegen::common::ContainerVersion};

pub(crate) trait Container<D, I>
where
    Self: Sized + Deref<Target = VersionedContainer<I>>,
{
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
