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

/// A versioned variant, which contains contains common [`Variant`] data and a
/// chain of actions.
///
/// The chain of action maps versions to an action and the appropriate variant
/// name. Additionally, the [`Variant`] data can be used to forward attributes,
/// generate documentation, etc.
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
    fn common_attributes_owned(self) -> ItemAttributes {
        self.common
    }

    fn common_attributes(&self) -> &ItemAttributes {
        &self.common
    }

    fn original_attributes(&self) -> &Vec<syn::Attribute> {
        &self.attrs
    }
}

impl Named for Variant {
    fn cleaned_ident(&self) -> Ident {
        remove_deprecated_variant_prefix(self.ident())
    }

    fn ident(&self) -> &Ident {
        &self.ident
    }
}

impl VersionedVariant {
    /// Creates a new versioned variant.
    ///
    /// Internally this calls [`VersionedItem::new`] to handle most of the
    /// common creation code.
    pub(crate) fn new(
        variant: Variant,
        container_attributes: &ContainerAttributes,
    ) -> syn::Result<Self> {
        let item = VersionedItem::<_, VariantAttributes>::new(variant, container_attributes)?;
        Ok(Self(item))
    }

    /// Generates tokens to be used in a container definition.
    pub(crate) fn generate_for_container(
        &self,
        container_version: &ContainerVersion,
    ) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;

        match &self.chain {
            // NOTE (@Techassi): https://rust-lang.github.io/rust-clippy/master/index.html#/expect_fun_call
            Some(chain) => match chain.get(&container_version.inner).unwrap_or_else(|| {
                panic!(
                    "internal error: chain must contain container version {}",
                    container_version.inner
                )
            }) {
                ItemStatus::Addition { ident, .. } => Some(quote! {
                    #(#original_attributes)*
                    #ident,
                }),
                ItemStatus::Change { to_ident, .. } => Some(quote! {
                    #(#original_attributes)*
                    #to_ident,
                }),
                ItemStatus::Deprecation { ident, .. } => Some(quote! {
                    #(#original_attributes)*
                    #[deprecated]
                    #ident,
                }),
                ItemStatus::NoChange(ident) => Some(quote! {
                    #(#original_attributes)*
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
                    #(#original_attributes)*
                    #variant_ident,
                })
            }
        }
    }

    /// Generates tokens to be used in a [`From`] implementation.
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
                (_, ItemStatus::Addition { .. }) => quote! {},
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
}
