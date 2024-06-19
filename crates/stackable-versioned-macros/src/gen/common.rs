use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use syn::Ident;

use crate::attrs::container::ContainerAttributes;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    pub(crate) deprecated: bool,
    pub(crate) skip_from: bool,
    pub(crate) inner: Version,
    pub(crate) ident: Ident,
}

impl From<&ContainerAttributes> for Vec<ContainerVersion> {
    fn from(attributes: &ContainerAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| ContainerVersion {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: Ident::new(&v.name.to_string(), Span::call_site()),
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect()
    }
}

pub(crate) trait VersionedContainer {
    type Container;
    type Data;

    fn new(ident: Ident, data: Self::Data, attributes: ContainerAttributes) -> Self::Container;
    fn generate_tokens(&self) -> TokenStream;
}

pub(crate) struct Container<T: Sized> {
    /// The ident, or name, of the versioned enum.
    pub(crate) ident: Ident,

    /// List of declared versions for this enum. Each version, except the
    /// latest, generates a definition with appropriate fields.
    pub(crate) versions: Vec<ContainerVersion>,

    pub(crate) items: Vec<T>,

    /// The name of the enum used in `From` implementations.
    pub(crate) from_ident: Ident,
    pub(crate) skip_from: bool,
}
