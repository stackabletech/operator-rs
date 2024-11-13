use std::ops::Not;

use darling::{FromAttributes, Result};
use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemEnum;

use crate::{
    attrs::container::NestedContainerAttributes,
    codegen::{
        changes::Neighbors,
        container::{CommonContainerData, Container, ContainerIdents, ContainerOptions},
        item::VersionedVariant,
        ItemStatus, StandaloneContainerAttributes, VersionDefinition,
    },
};

impl Container {
    pub(crate) fn new_standalone_enum(
        item_enum: ItemEnum,
        attributes: StandaloneContainerAttributes,
        versions: &[VersionDefinition],
    ) -> Result<Self> {
        let mut versioned_variants = Vec::new();
        for variant in item_enum.variants {
            let mut versioned_variant = VersionedVariant::new(variant, versions)?;
            versioned_variant.insert_container_versions(versions);
            versioned_variants.push(versioned_variant);
        }

        let options = ContainerOptions {
            kubernetes_options: None,
            skip_from: attributes
                .common_root_arguments
                .options
                .skip
                .map_or(false, |s| s.from.is_present()),
        };

        let idents: ContainerIdents = item_enum.ident.into();

        let common = CommonContainerData {
            original_attributes: item_enum.attrs,
            options,
            idents,
        };

        Ok(Self::Enum(Enum {
            variants: versioned_variants,
            common,
        }))
    }

    // TODO (@Techassi): See what can be unified into a single 'new' function
    pub(crate) fn new_enum_nested(
        item_enum: ItemEnum,
        versions: &[VersionDefinition],
    ) -> Result<Self> {
        let attributes = NestedContainerAttributes::from_attributes(&item_enum.attrs)?;

        let mut versioned_variants = Vec::new();
        for variant in item_enum.variants {
            let mut versioned_variant = VersionedVariant::new(variant, versions)?;
            versioned_variant.insert_container_versions(versions);
            versioned_variants.push(versioned_variant);
        }

        let options = ContainerOptions {
            kubernetes_options: None,
            skip_from: attributes
                .options
                .skip
                .map_or(false, |s| s.from.is_present()),
        };

        let idents: ContainerIdents = item_enum.ident.into();

        let common = CommonContainerData {
            original_attributes: item_enum.attrs,
            options,
            idents,
        };

        Ok(Self::Enum(Enum {
            variants: versioned_variants,
            common,
        }))
    }
}

pub(crate) struct Enum {
    /// List of variants defined in the original enum. How, and if, an item
    /// should generate code, is decided by the currently generated version.
    pub(crate) variants: Vec<VersionedVariant>,
    pub(crate) common: CommonContainerData,
}

impl Enum {
    pub(crate) fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        let original_attributes = &self.common.original_attributes;
        let ident = &self.common.idents.original;

        let mut variants = TokenStream::new();
        for variant in &self.variants {
            variants.extend(variant.generate_for_container(version));
        }

        quote! {
            #(#original_attributes)*
            pub enum #ident {
                #variants
            }
        }
    }

    pub(crate) fn generate_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        is_nested: bool,
    ) -> Option<TokenStream> {
        match next_version {
            Some(next_version) => {
                let enum_ident = &self.common.idents.original;
                let from_ident = &self.common.idents.from;

                let next_version_ident = &next_version.ident;
                let version_ident = &version.ident;

                let mut variants = TokenStream::new();
                for variant in &self.variants {
                    variants.extend(variant.generate_for_from_impl(
                        version,
                        next_version,
                        enum_ident,
                    ));
                }

                // Include allow(deprecated) only when this or the next version is
                // deprecated. Also include it, when a variant in this or the next
                // version is deprecated.
                let allow_attribute = (version.deprecated
                    || next_version.deprecated
                    || self.is_any_variant_deprecated(version)
                    || self.is_any_variant_deprecated(next_version))
                .then_some(quote! { #[allow(deprecated)] });

                // Only add the #[automatically_derived] attribute only if this impl is used
                // outside of a module (in standalone mode).
                let automatically_derived =
                    is_nested.not().then(|| quote! {#[automatically_derived]});

                Some(quote! {
                    #automatically_derived
                    #allow_attribute
                    impl ::std::convert::From<#version_ident::#enum_ident> for #next_version_ident::#enum_ident {
                        fn from(#from_ident: #version_ident::#enum_ident) -> Self {
                            match #from_ident {
                                #variants
                            }
                        }
                    }
                })
            }
            None => None,
        }
    }

    /// Returns whether any variant is deprecated in the provided `version`.
    fn is_any_variant_deprecated(&self, version: &VersionDefinition) -> bool {
        // First, iterate over all variants. Any will return true if any of the
        // function invocations return true. If a field doesn't have a chain,
        // we can safely default to false (unversioned fields cannot be
        // deprecated). Then we retrieve the status of the field and ensure it
        // is deprecated.
        self.variants.iter().any(|f| {
            f.changes.as_ref().map_or(false, |c| {
                c.value_is(&version.inner, |a| {
                    matches!(
                        a,
                        ItemStatus::Deprecation { .. }
                            | ItemStatus::NoChange {
                                previously_deprecated: true,
                                ..
                            }
                    )
                })
            })
        })
    }
}
