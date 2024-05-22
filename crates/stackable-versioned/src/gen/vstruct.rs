use darling::FromField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DataStruct, Ident, Result};

use crate::{
    attrs::{container::ContainerAttributes, field::FieldAttributes},
    gen::{field::VersionedField, version::ContainerVersion, ToTokensExt},
};

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
#[derive(Debug)]
pub(crate) struct VersionedStruct {
    /// The ident, or name, of the versioned struct.
    pub(crate) ident: Ident,

    /// List of declared versions for this struct. Each version, except the
    /// latest, generates a definition with appropriate fields.
    pub(crate) versions: Vec<ContainerVersion>,

    /// List of fields defined in the base struct. How, and if, a field should
    /// generate code, is decided by the currently generated version.
    pub(crate) fields: Vec<VersionedField>,
}

impl ToTokensExt<bool> for VersionedStruct {
    fn to_tokens(&self, generate_modules: bool) -> Option<TokenStream> {
        // TODO (@Techassi): This unwrap should be fine, should we expect here?
        let mut versions = self.versions.clone();
        versions.pop().unwrap();
        let mut versions = versions.iter().peekable();

        let mut tokens = TokenStream::new();

        // TODO (@Techassi): Move this into own functions
        while let Some(version) = versions.next() {
            let mut field_tokens = TokenStream::new();

            for field in &self.fields {
                field_tokens.extend(field.to_tokens(version));
            }

            let module_name = format_ident!("{version}", version = version.inner.to_string());
            let deprecated_attr = version.deprecated.then_some(quote! {#[deprecated]});
            let struct_name = &self.ident;

            let struct_tokens = quote! {
                pub struct #struct_name {
                    #field_tokens
                }
            };

            // Only generate modules when asked to do so by the caller. This
            // enables us the support attribute macros to generate code for
            // multiple versioned containers in a single file (no module name
            // collition).
            if generate_modules {
                // Only generate a module when there is at least one more
                // version. This skips generating a module for the latest
                // version, because the base struct always represents the
                // latest version.
                tokens.extend(quote! {
                    #[automatically_derived]
                    #deprecated_attr
                    pub mod #module_name {
                        #struct_tokens
                    }
                });

                if let Some(next) = versions.peek() {
                    // Generate From<THIS> for NEXT impls
                    let next_module = format_ident!("{}", next.inner.to_string());

                    let from_impl_tokens = quote! {
                        #[automatically_derived]
                        impl From<#module_name::#struct_name> for #next_module::#struct_name {
                            fn from(from: #module_name::#struct_name) -> Self {
                                todo!();
                            }
                        }
                    };

                    tokens.extend(from_impl_tokens);
                } else {
                    let from_impl_tokens = quote! {
                        #[automatically_derived]
                        impl From<#module_name::#struct_name> for #struct_name {
                            fn from(from: #module_name::#struct_name) -> Self {
                                todo!();
                            }
                        }
                    };

                    tokens.extend(from_impl_tokens);
                }
            } else {
                tokens.extend(struct_tokens)
            }
        }

        Some(tokens)
    }
}

impl VersionedStruct {
    pub(crate) fn new(
        ident: Ident,
        data: DataStruct,
        attributes: ContainerAttributes,
    ) -> Result<Self> {
        // Convert the raw version attributes into a container version.
        let versions = attributes
            .versions
            .iter()
            .map(|v| ContainerVersion {
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut fields = Vec::new();

        for field in data.fields {
            let attrs = FieldAttributes::from_field(&field)?;
            attrs.validate_versions(&attributes, &field)?;

            let mut versioned_field = VersionedField::new(field, attrs)?;
            versioned_field.insert_container_versions(&versions);
            fields.push(versioned_field);
        }

        Ok(Self {
            ident,
            versions,
            fields,
        })
    }
}
