use std::ops::Not;

use darling::{FromAttributes, Result};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, ItemEnum};

use crate::{
    attrs::container::NestedContainerAttributes,
    codegen::{
        ItemStatus, StandaloneContainerAttributes, VersionDefinition,
        changes::Neighbors,
        container::{CommonContainerData, Container, ContainerIdents, ContainerOptions},
        item::VersionedVariant,
    },
};

impl Container {
    pub fn new_standalone_enum(
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
            kubernetes_arguments: None,
            skip_from: attributes
                .common
                .options
                .skip
                .is_some_and(|s| s.from.is_present()),
        };

        let idents = ContainerIdents::from(item_enum.ident, None);

        let common = CommonContainerData {
            original_attributes: item_enum.attrs,
            options,
            idents,
        };

        Ok(Self::Enum(Enum {
            generics: item_enum.generics,
            variants: versioned_variants,
            common,
        }))
    }

    // TODO (@Techassi): See what can be unified into a single 'new' function
    pub fn new_enum_nested(item_enum: ItemEnum, versions: &[VersionDefinition]) -> Result<Self> {
        let attributes = NestedContainerAttributes::from_attributes(&item_enum.attrs)?;

        let mut versioned_variants = Vec::new();
        for variant in item_enum.variants {
            let mut versioned_variant = VersionedVariant::new(variant, versions)?;
            versioned_variant.insert_container_versions(versions);
            versioned_variants.push(versioned_variant);
        }

        let options = ContainerOptions {
            kubernetes_arguments: None,
            skip_from: attributes.options.skip.is_some_and(|s| s.from.is_present()),
        };

        let idents = ContainerIdents::from(item_enum.ident, None);

        let common = CommonContainerData {
            original_attributes: item_enum.attrs,
            options,
            idents,
        };

        Ok(Self::Enum(Enum {
            generics: item_enum.generics,
            variants: versioned_variants,
            common,
        }))
    }
}

/// A versioned enum.
pub struct Enum {
    /// List of variants defined in the original enum. How, and if, an item
    /// should generate code, is decided by the currently generated version.
    pub variants: Vec<VersionedVariant>,

    /// Common container data which is shared between enums and structs.
    pub common: CommonContainerData,

    /// Generic types of the enum
    pub generics: Generics,
}

// Common token generation
impl Enum {
    /// Generates code for the enum definition.
    pub fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        let where_clause = self.generics.where_clause.as_ref();
        let type_generics = &self.generics;

        let original_attributes = &self.common.original_attributes;
        let ident = &self.common.idents.original;
        let version_docs = &version.docs;

        let mut variants = TokenStream::new();
        for variant in &self.variants {
            variants.extend(variant.generate_for_container(version));
        }

        quote! {
            #(#[doc = #version_docs])*
            #(#original_attributes)*
            pub enum #ident #type_generics #where_clause {
                #variants
            }
        }
    }

    /// Generates code for the `From<Version> for NextVersion` implementation.
    pub fn generate_upgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        if version.skip_from || self.common.options.skip_from {
            return None;
        }

        match next_version {
            Some(next_version) => {
                // TODO (@Techassi): Support generic types which have been removed in newer versions,
                // but need to exist for older versions How do we represent that? Because the
                // defined struct always represents the latest version. I guess we could generally
                // advise against using generic types, but if you have to, avoid removing it in
                // later versions.
                let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
                let from_enum_ident = &self.common.idents.parameter;
                let enum_ident = &self.common.idents.original;

                let for_module_ident = &next_version.idents.module;
                let from_module_ident = &version.idents.module;

                let variants: TokenStream = self
                    .variants
                    .iter()
                    .filter_map(|v| {
                        v.generate_for_upgrade_from_impl(version, next_version, enum_ident)
                    })
                    .collect();

                // Include allow(deprecated) only when this or the next version is
                // deprecated. Also include it, when a variant in this or the next
                // version is deprecated.
                let allow_attribute = (version.deprecated.is_some()
                    || next_version.deprecated.is_some()
                    || self.is_any_variant_deprecated(version)
                    || self.is_any_variant_deprecated(next_version))
                .then_some(quote! { #[allow(deprecated)] });

                // Only add the #[automatically_derived] attribute only if this impl is used
                // outside of a module (in standalone mode).
                let automatically_derived = add_attributes
                    .not()
                    .then(|| quote! {#[automatically_derived]});

                Some(quote! {
                    #automatically_derived
                    #allow_attribute
                    impl #impl_generics ::std::convert::From<#from_module_ident::#enum_ident #type_generics> for #for_module_ident::#enum_ident #type_generics
                        #where_clause
                    {
                        fn from(#from_enum_ident: #from_module_ident::#enum_ident #type_generics) -> Self {
                            match #from_enum_ident {
                                #variants
                            }
                        }
                    }
                })
            }
            None => None,
        }
    }

    pub fn generate_downgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        if version.skip_from || self.common.options.skip_from {
            return None;
        }

        match next_version {
            Some(next_version) => {
                let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
                let from_enum_ident = &self.common.idents.parameter;
                let enum_ident = &self.common.idents.original;

                let from_module_ident = &next_version.idents.module;
                let for_module_ident = &version.idents.module;

                let variants: TokenStream = self
                    .variants
                    .iter()
                    .filter_map(|v| {
                        v.generate_for_downgrade_from_impl(version, next_version, enum_ident)
                    })
                    .collect();

                // Include allow(deprecated) only when this or the next version is
                // deprecated. Also include it, when a variant in this or the next
                // version is deprecated.
                let allow_attribute = (version.deprecated.is_some()
                    || next_version.deprecated.is_some()
                    || self.is_any_variant_deprecated(version)
                    || self.is_any_variant_deprecated(next_version))
                .then_some(quote! { #[allow(deprecated)] });

                // Only add the #[automatically_derived] attribute only if this impl is used
                // outside of a module (in standalone mode).
                let automatically_derived = add_attributes
                    .not()
                    .then(|| quote! {#[automatically_derived]});

                Some(quote! {
                    #automatically_derived
                    #allow_attribute
                    impl #impl_generics ::std::convert::From<#from_module_ident::#enum_ident #type_generics> for #for_module_ident::#enum_ident #type_generics
                        #where_clause
                    {
                        fn from(#from_enum_ident: #from_module_ident::#enum_ident #type_generics) -> Self {
                            match #from_enum_ident {
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
        // First, iterate over all variants. The `any` function will return true
        // if any of the function invocations return true. If a variant doesn't
        // have a chain, we can safely default to false (unversioned variants
        // cannot be deprecated). Then we retrieve the status of the variant and
        // ensure it is deprecated.
        self.variants.iter().any(|f| {
            f.changes.as_ref().is_some_and(|c| {
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
