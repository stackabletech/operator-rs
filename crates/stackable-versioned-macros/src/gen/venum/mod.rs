use std::ops::Deref;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DataEnum, Error, Ident};

use crate::{
    attrs::common::ContainerAttributes,
    gen::{
        common::{format_container_from_ident, Container, ContainerVersion, VersionedContainer},
        venum::variant::VersionedVariant,
    },
};

mod variant;

#[derive(Debug)]
pub(crate) struct VersionedEnum(VersionedContainer<VersionedVariant>);

impl Deref for VersionedEnum {
    type Target = VersionedContainer<VersionedVariant>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Container<DataEnum, VersionedVariant> for VersionedEnum {
    fn new(ident: Ident, data: DataEnum, attributes: ContainerAttributes) -> syn::Result<Self> {
        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the attributes for every variant from the raw token
        // stream and also validate that each variant action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for variant in data.variants {
            let mut versioned_field = VersionedVariant::new(variant, &attributes)?;
            versioned_field.insert_container_versions(&versions);
            items.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all variants for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.

            // TODO (@Techassi): Report which variant(s) use a duplicate ident and
            // also hint what can be done to fix it based on the variant action /
            // status.

            if !items.iter().map(|f| f.get_ident(version)).all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("struct contains renamed fields which collide with other fields in version {version}", version = version.inner),
                ));
            }
        }

        let from_ident = format_container_from_ident(&ident);

        Ok(Self(VersionedContainer {
            skip_from: attributes
                .options
                .skip
                .map_or(false, |s| s.from.is_present()),
            from_ident,
            versions,
            items,
            ident,
        }))
    }

    fn generate_tokens(&self) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            token_stream.extend(self.generate_version(version, versions.peek().copied()));
        }

        token_stream
    }
}

impl VersionedEnum {
    fn generate_version(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let enum_name = &self.ident;

        // Generate variants of the enum for `version`.
        let variants = self.generate_enum_variants(version);

        // TODO (@Techassi): Make the generation of the module optional to
        // enable the attribute macro to be applied to a module which
        // generates versioned versions of all contained containers.

        let version_ident = &version.ident;

        let deprecated_note = format!("Version {version} is deprecated", version = version_ident);
        let deprecated_attr = version
            .deprecated
            .then_some(quote! {#[deprecated = #deprecated_note]});

        // Generate tokens for the module and the contained struct
        token_stream.extend(quote! {
            #[automatically_derived]
            #deprecated_attr
            pub mod #version_ident {
                pub enum #enum_name {
                    #variants
                }
            }
        });

        // Generate the From impl between this `version` and the next one.
        if !self.skip_from && !version.skip_from {
            token_stream.extend(self.generate_from_impl(version, next_version));
        }

        token_stream
    }

    fn generate_enum_variants(&self, version: &ContainerVersion) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for variant in &self.items {
            token_stream.extend(variant.generate_for_container(version));
        }

        token_stream
    }

    fn generate_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let module_name = &version.ident;

            let from_ident = &self.from_ident;
            let enum_ident = &self.ident;

            let mut variants = TokenStream::new();

            for item in &self.items {
                variants.extend(item.generate_for_from_impl(
                    module_name,
                    next_module_name,
                    version,
                    next_version,
                    enum_ident,
                ))
            }

            // TODO (@Techassi): Be a little bit more clever about when to include
            // the #[allow(deprecated)] attribute.
            return quote! {
                #[automatically_derived]
                #[allow(deprecated)]
                impl From<#module_name::#enum_ident> for #next_module_name::#enum_ident {
                    fn from(#from_ident: #module_name::#enum_ident) -> Self {
                        match #from_ident {
                            #variants
                        }
                    }
                }
            };
        }

        quote! {}
    }
}
