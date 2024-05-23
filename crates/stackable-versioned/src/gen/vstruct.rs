use darling::FromField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
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

impl ToTokens for VersionedStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut versions = self.versions.iter().peekable();
        let struct_name = &self.ident;

        while let Some(version) = versions.next() {
            let mut fields = TokenStream::new();

            for field in &self.fields {
                fields.extend(field.to_tokens_for_version(version));
            }

            // TODO (@Techassi): Make the generation of the module optional to
            // enable the attribute macro to be applied to a module which
            // generates versioned versions of all contained containers.

            let deprecated_attr = version.deprecated.then_some(quote! {#[deprecated]});
            let module_name = format_ident!("{version}", version = version.inner.to_string());

            tokens.extend(quote! {
                #[automatically_derived]
                #deprecated_attr
                pub mod #module_name {

                    pub struct #struct_name {
                        #fields
                    }
                }
            });
        }

        // Special handling for the last (and thus latest) version
        let module_name = format_ident!(
            "{version}",
            version = self.versions.last().unwrap().inner.to_string()
        );
        tokens.extend(quote! {
            pub type #struct_name = #module_name::#struct_name;
        })
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
