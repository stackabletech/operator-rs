use std::ops::Deref;

use darling::FromField;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::{spanned::Spanned, DataStruct, Error, Ident, Result};

use crate::{
    attrs::{
        container::ContainerAttributes,
        field::{FieldAction, FieldAttributes},
    },
    gen::{field::VersionedField, version::Version},
};

pub(crate) struct VersionedStruct {
    /// Stores individual versions of a single struct. Each version tracks field
    /// actions, which describe if the field was added, renamed or deprecated in
    /// that version. Fields which are not versioned, are included in every
    /// version of the struct.
    versions: Vec<Version>,
}

impl ToTokens for VersionedStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // Iterate over each individual version to generate the version modules
        // and the appropriate nested struct.
        for (index, version) in self.versions.iter().enumerate() {
            tokens.extend(version.to_token_stream());

            // If the last version (currently assumes versions are declared in
            // ascending order) is encountered, also generate the latest module
            // which always points to the highest / latest version.
            if index + 1 == self.versions.len() {
                let latest_version_name = &version.module;

                tokens.extend(quote! {
                    #[automatically_derived]
                    pub mod latest {
                        pub use super::#latest_version_name::*;
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
        // First, collect all declared versions and validate that each version
        // is unique and isn't declared multiple times.
        let mut versions = attributes
            .versions
            .iter()
            .map(|v| Version::new(ident.clone(), v.name.deref().clone()))
            .collect::<Vec<_>>();

        for field in data.fields {
            // Iterate over all fields of the struct and gather field attributes.
            // Next, make sure only valid combinations of field actions are
            // declared. Using the action and the field data, a VersionField
            // can be created.
            let field_attributes = FieldAttributes::from_field(&field)?;
            let field_action = FieldAction::try_from(field_attributes)?;

            // Validate, that the field action uses a version which is declared
            // by the container attribute.
            if let Some(version) = versions
                .iter_mut()
                .find(|v| v.name == field_action.since().unwrap_or_default())
            {
                version
                    .fields
                    .push(VersionedField::new(field, field_action));

                continue;
            }

            // At this point the version specified in the action is not in the
            // set of declared versions and thus an error is returned.
            return Err(Error::new(
                field.span(),
                format!("field action `{}` contains version which is not declared via `#[versioned(version)]`", field_action),
            ));
        }

        Ok(Self { versions })
    }
}
