use std::ops::Deref;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DataStruct, Error, Ident};

use crate::{
    attrs::common::ContainerAttributes,
    codegen::{
        common::{
            format_container_from_ident, Container, ContainerInput, ContainerVersion, Item,
            VersionedContainer,
        },
        vstruct::field::VersionedField,
    },
};

pub(crate) mod field;

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
#[derive(Debug)]
pub(crate) struct VersionedStruct(VersionedContainer<VersionedField>);

impl Deref for VersionedStruct {
    type Target = VersionedContainer<VersionedField>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Container<DataStruct, VersionedField> for VersionedStruct {
    fn new(
        input: ContainerInput,
        data: DataStruct,
        attributes: ContainerAttributes,
    ) -> syn::Result<Self> {
        let ContainerInput {
            original_attributes,
            visibility,
            ident,
        } = input;

        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for field in data.fields {
            let mut versioned_field = VersionedField::new(field, &attributes)?;
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
            original_attributes,
            visibility,
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

impl VersionedStruct {
    fn generate_version(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();

        let original_attributes = &self.original_attributes;
        let visibility = &self.visibility;
        let struct_name = &self.ident;

        // Generate fields of the struct for `version`.
        let fields = self.generate_struct_fields(version);

        // TODO (@Techassi): Make the generation of the module optional to
        // enable the attribute macro to be applied to a module which
        // generates versioned versions of all contained containers.

        let version_ident = &version.ident;

        let deprecated_note = format!("Version {version} is deprecated", version = version_ident);
        let deprecated_attr = version
            .deprecated
            .then_some(quote! {#[deprecated = #deprecated_note]});

        let mut version_specific_docs = TokenStream::new();
        for (i, doc) in version.version_specific_docs.iter().enumerate() {
            if i == 0 {
                // Prepend an empty line to clearly separate the version
                // specific docs.
                version_specific_docs.extend(quote! {
                    #[doc = ""]
                })
            }
            version_specific_docs.extend(quote! {
                #[doc = #doc]
            })
        }

        // Generate tokens for the module and the contained struct
        token_stream.extend(quote! {
            #[automatically_derived]
            #deprecated_attr
            #visibility mod #version_ident {
                #(#original_attributes)*
                #version_specific_docs
                pub struct #struct_name {
                    #fields
                }
            }
        });

        // Generate the From impl between this `version` and the next one.
        if !self.skip_from && !version.skip_from {
            token_stream.extend(self.generate_from_impl(version, next_version));
        }

        token_stream
    }

    fn generate_struct_fields(&self, version: &ContainerVersion) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for item in &self.items {
            token_stream.extend(item.generate_for_container(version));
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
            let struct_ident = &self.ident;

            let fields = self.generate_from_fields(version, next_version, from_ident);

            // TODO (@Techassi): Be a little bit more clever about when to include
            // the #[allow(deprecated)] attribute.
            return quote! {
                #[automatically_derived]
                #[allow(deprecated)]
                impl From<#module_name::#struct_ident> for #next_module_name::#struct_ident {
                    fn from(#from_ident: #module_name::#struct_ident) -> Self {
                        Self {
                            #fields
                        }
                    }
                }
            };
        }

        quote! {}
    }

    fn generate_from_fields(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for item in &self.items {
            token_stream.extend(item.generate_for_from_impl(version, next_version, from_ident))
        }

        token_stream
    }
}
