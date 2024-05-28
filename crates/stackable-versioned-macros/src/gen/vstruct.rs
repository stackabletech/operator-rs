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
                ident: format_ident!("{version}", version = v.name.to_string()),
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

    /// This generates the complete code for a single versioned struct.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the struct with the appropriate fields. Additionally, it
    /// generated `From` implementations, which enable conversion from an older
    /// to a newer version.
    pub(crate) fn generate_tokens(&self) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            token_stream.extend(self.generate_version(version, versions.peek().copied()));
        }

        token_stream
    }

    pub(crate) fn generate_version(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        let mut token_stream = TokenStream::new();
        let struct_name = &self.ident;

        // Generate fields of the struct for `version`.
        let fields = self.generate_struct_fields(version);

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
                pub struct #struct_name {
                    #fields
                }
            }
        });

        // Generate the From impl between this `version` and the next one.
        token_stream.extend(self.generate_from_impl(version, next_version));

        token_stream
    }

    pub(crate) fn generate_struct_fields(&self, version: &ContainerVersion) -> TokenStream {
        let mut token_stream = TokenStream::new();

        for field in &self.fields {
            token_stream.extend(field.to_tokens(version));
        }

        token_stream
    }

    pub(crate) fn generate_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: Option<&ContainerVersion>,
    ) -> TokenStream {
        if let Some(next_version) = next_version {
            let next_module_name = &next_version.ident;
            let module_name = &version.ident;
            let struct_name = &self.ident;

            return quote! {
                #[automatically_derived]
                impl From<#module_name::#struct_name> for #next_module_name::#struct_name {
                    fn from(from: #module_name::#struct_name) -> Self {
                        todo!();
                    }
                }
            };
        }

        quote! {}
    }
}
