use std::ops::Deref;

use darling::FromField;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{spanned::Spanned, DataStruct, Error, Ident, Result};

use crate::attrs::{
    container::ContainerAttributes,
    field::{FieldAction, FieldAttributes},
};

pub(crate) struct Version {
    name: String,
    _deprecated: bool,
}

/// Stores individual versions of a single struct. Each version tracks field
/// actions, which describe if the field was added, renamed or deprecated in
/// that version. Fields which are not versioned, are included in every
/// version of the struct.
pub(crate) struct VersionedStruct {
    pub(crate) _ident: Ident,

    pub(crate) _actions: Vec<FieldAction>,
    pub(crate) _versions: Vec<Version>,
}

impl ToTokens for VersionedStruct {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        // Iterate over each individual version to generate the version modules
        // and the appropriate nested struct.

        // for (index, version) in self.versions.iter().enumerate() {
        //     tokens.extend(version.to_token_stream());

        //     // If the last version (currently assumes versions are declared in
        //     // ascending order) is encountered, also generate the latest module
        //     // which always points to the highest / latest version.
        //     if index + 1 == self.versions.len() {
        //         let latest_version_name = &version.module;

        //         tokens.extend(quote! {
        //             #[automatically_derived]
        //             pub mod latest {
        //                 pub use super::#latest_version_name::*;
        //             }
        //         })
        //     }
        // }
        todo!()
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
            .cloned()
            .map(|v| Version {
                name: v.name.deref().clone(),
                _deprecated: v.deprecated.is_present(),
            })
            .collect::<Vec<_>>();

        let mut actions = Vec::new();

        for field in &data.fields {
            // Iterate over all fields of the struct and gather field attributes.
            // Next, make sure only valid combinations of field actions are
            // declared. Using the action and the field data, a VersionField
            // can be created.
            let field_attributes = FieldAttributes::from_field(field)?;
            let field_action = FieldAction::try_from(field_attributes)?;

            // Validate, that the field action uses a version which is declared
            // by the container attribute. If there is no attribute attached to
            // the field, it is also valid.
            match field_action.since() {
                Some(since) => {
                    if versions.iter().any(|v| v.name == since) {
                        actions.push(field_action);
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
                None => continue,
            }
        }

        Ok(Self {
            _versions: versions,
            _actions: actions,
            _ident: ident,
        })
    }
}
