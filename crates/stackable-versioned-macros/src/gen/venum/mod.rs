use std::ops::Deref;

use darling::FromVariant;
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DataEnum, Error, Ident};

use crate::{
    attrs::{common::ContainerAttributes, variant::VariantAttributes},
    gen::{
        common::{
            format_container_from_ident, Container, ContainerVersion, Item, VersionedContainer,
        },
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

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for variant in data.variants {
            let attrs = VariantAttributes::from_variant(&variant)?;
            attrs.validate_versions(&attributes, &variant)?;

            let mut versioned_field = VersionedVariant::new(variant, attrs);
            versioned_field.insert_container_versions(&versions);
            items.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all fields for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.

            // TODO (@Techassi): Report which field(s) use a duplicate ident and
            // also hint what can be done to fix it based on the field action /
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

        let deprecated_attr = version.deprecated.then_some(quote! {#[deprecated]});
        let module_name = &version.ident;

        // Generate tokens for the module and the contained struct
        token_stream.extend(quote! {
            #[automatically_derived]
            #deprecated_attr
            pub mod #module_name {
                pub enum #enum_name {
                    #variants
                }
            }
        });

        token_stream
    }

    fn generate_enum_variants(&self, version: &ContainerVersion) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for variant in &self.items {
            token_stream.extend(variant.generate_for_container(version));
        }

        token_stream
    }
}
