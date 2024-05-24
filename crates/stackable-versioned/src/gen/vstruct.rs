use convert_case::{Case, Casing};
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
        let versions = self.versions.iter().peekable();

        let module_name = self.ident.to_string().to_case(Case::Snake);
        let module_name = format_ident!("{module_name}");
        let alias_name = &self.ident;

        let mut struct_tokens = TokenStream::new();

        for version in versions {
            let mut field_tokens = TokenStream::new();

            for field in &self.fields {
                field_tokens.extend(field.to_tokens_for_version(version));
            }

            let deprecated_attr = version.deprecated.then_some(quote! {#[deprecated]});

            let struct_name = version.inner.to_string().to_case(Case::Pascal);
            let struct_name = format_ident!("{struct_name}");

            struct_tokens.extend(quote! {
                #deprecated_attr
                pub struct #struct_name {
                    #field_tokens
                }
            })
        }

        // Generate module with contents
        tokens.extend(quote! {
            #[automatically_derived]
            pub mod #module_name {
                #struct_tokens
            }
        });

        // Special handling for the last (and thus latest) version
        let struct_name = self
            .versions
            .last()
            .unwrap()
            .inner
            .to_string()
            .to_case(Case::Pascal);
        let struct_name = format_ident!("{struct_name}");

        tokens.extend(quote! {
            pub type #alias_name = #module_name::#struct_name;
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
