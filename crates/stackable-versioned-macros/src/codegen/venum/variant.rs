use std::ops::{Deref, DerefMut};

use darling::FromVariant;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Variant};

use crate::{
    attrs::{
        common::{ContainerAttributes, ItemAttributes},
        variant::VariantAttributes,
    },
    codegen::{
        chain::BTreeMapExt,
        common::{
            remove_deprecated_variant_prefix, Attributes, ContainerVersion, Item, ItemStatus,
            Named, VersionedItem,
        },
    },
};

#[derive(Debug)]
pub(crate) struct VersionedVariant(VersionedItem<Variant, VariantAttributes>);

impl Deref for VersionedVariant {
    type Target = VersionedItem<Variant, VariantAttributes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VersionedVariant {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<&Variant> for VariantAttributes {
    type Error = darling::Error;

    fn try_from(variant: &Variant) -> Result<Self, Self::Error> {
        Self::from_variant(variant)
    }
}

impl Attributes for VariantAttributes {
    fn common_attrs_owned(self) -> ItemAttributes {
        self.common
    }

    fn common_attrs(&self) -> &ItemAttributes {
        &self.common
    }
}

impl Named for Variant {
    fn cleaned_ident(&self) -> Ident {
        let ident = self.ident();
        remove_deprecated_variant_prefix(ident)
    }

    fn ident(&self) -> &Ident {
        &self.ident
    }
}

// TODO (@Techassi): Figure out a way to be able to only write the following code
// once for both a versioned field and variant, because the are practically
// identical.

impl VersionedVariant {
    pub(crate) fn new(
        variant: Variant,
        container_attributes: &ContainerAttributes,
    ) -> syn::Result<Self> {
        let item = VersionedItem::<_, VariantAttributes>::new(variant, container_attributes)?;
        Ok(Self(item))
    }

    pub(crate) fn generate_for_container(
        &self,
        container_version: &ContainerVersion,
    ) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => match chain
                .get(&container_version.inner)
                .expect("internal error: chain must contain container version")
            {
                ItemStatus::Added { ident, .. } => Some(quote! {
                    #ident,
                }),
                ItemStatus::Renamed { to, .. } => Some(quote! {
                    #to,
                }),
                ItemStatus::Deprecated { ident, .. } => Some(quote! {
                    #[deprecated]
                    #ident,
                }),
                ItemStatus::NoChange(ident) => Some(quote! {
                    #ident,
                }),
                ItemStatus::NotPresent => None,
            },
            None => {
                // If there is no chain of variant actions, the variant is not
                // versioned and code generation is straight forward.
                // Unversioned variants are always included in versioned enums.
                let variant_ident = &self.inner.ident;

                Some(quote! {
                    #variant_ident,
                })
            }
        }
    }

    pub(crate) fn generate_for_from_impl(
        &self,
        module_name: &Ident,
        next_module_name: &Ident,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        enum_ident: &Ident,
    ) -> TokenStream {
        match &self.chain {
            Some(chain) => match (
                chain.get_expect(&version.inner),
                chain.get_expect(&next_version.inner),
            ) {
                (_, ItemStatus::Added { .. }) => quote! {},
                (old, next) => {
                    let old_variant_ident = old
                        .get_ident()
                        .expect("internal error: old variant must have a name");
                    let next_variant_ident = next
                        .get_ident()
                        .expect("internal error: next variant must have a name");

                    quote! {
                        #module_name::#enum_ident::#old_variant_ident => #next_module_name::#enum_ident::#next_variant_ident,
                    }
                }
            },
            None => {
                let variant_ident = &self.inner.ident;

                quote! {
                    #module_name::#enum_ident::#variant_ident => #next_module_name::#enum_ident::#variant_ident,
                }
            }
        }
    }

    pub(crate) fn get_ident(&self, version: &ContainerVersion) -> Option<&syn::Ident> {
        match &self.chain {
            Some(chain) => chain
                .get(&version.inner)
                .expect("internal error: chain must contain container version")
                .get_ident(),
            None => Some(&self.inner.ident),
        }
    }
}
