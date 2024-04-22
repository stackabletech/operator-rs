use darling::FromField;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{spanned::Spanned, DataStruct, Error, Ident, Result};

use crate::{
    attrs::{
        container::ContainerAttributes,
        field::{FieldAction, FieldAttributes},
    },
    gen::{field::VersionedField, version::ContainerVersion, ToTokensExt},
};

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
pub(crate) struct VersionedStruct {
    pub(crate) _ident: Ident,

    pub(crate) _versions: Vec<ContainerVersion>,
    pub(crate) _fields: Vec<VersionedField>,
}

impl ToTokens for VersionedStruct {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        let mut versions = self._versions.iter().peekable();

        while let Some(version) = versions.next() {
            let mut fields = TokenStream::new();

            for field in &self._fields {
                fields.extend(field.to_tokens_for_version(version))
            }

            // TODO (@Techassi): Make the generation of the module optional to
            // enable the attribute macro to be applied to a module which
            // generates versioned versions of all contained containers.

            let module_name = format_ident!("{}", version.inner.to_string());
            let struct_name = &self._ident;

            _tokens.extend(quote! {
                pub mod #module_name {
                    pub struct #struct_name {
                        #fields
                    }
                }
            });

            // If there is no next version, we know we just generated the latest
            // version and thus we can add the 'latest' module.
            if versions.peek().is_none() {
                _tokens.extend(quote! {
                    pub mod latest {
                        pub use super::#module_name::*;
                    }
                })
            }
        }
    }
}

impl VersionedStruct {
    pub(crate) fn new(
        ident: Ident,
        data: DataStruct,
        attributes: ContainerAttributes,
    ) -> Result<Self> {
        // First, collect all declared versions and map them into a Version
        // struct.
        let versions = attributes
            .versions
            .iter()
            .map(|v| {
                let deprecated = v.deprecated.is_present();

                ContainerVersion {
                    _deprecated: deprecated,
                    inner: v.name,
                }
            })
            .collect::<Vec<_>>();

        let mut fields = Vec::new();

        for field in data.fields {
            // Iterate over all fields of the struct and gather field attributes.
            // Next, make sure only valid combinations of field actions are
            // declared. Using the action and the field data, a VersionField
            // can be created.
            let field_attributes = FieldAttributes::from_field(&field)?;
            let field_action = FieldAction::try_from(field_attributes)?;

            // Validate, that the field action uses a version which is declared
            // by the container attribute. If there is no attribute attached to
            // the field, it is also valid.
            match field_action.since() {
                Some(since) => {
                    if versions.iter().any(|v| v.inner == *since) {
                        fields.push(VersionedField::new(field, field_action));
                        continue;
                    }

                    // At this point the version specified in the action is not
                    // in the set of declared versions and thus an error is
                    // returned.
                    return Err(Error::new(
                        field.span(),
                        format!("field action `{}` contains version which is not declared via `#[versioned(version)]`", field_action),
                    ));
                }
                None => fields.push(VersionedField::new(field, field_action)),
            }
        }

        Ok(Self {
            _versions: versions,
            _fields: fields,
            _ident: ident,
        })
    }
}
