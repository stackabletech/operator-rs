use std::collections::BTreeMap;

use darling::{FromField, Result, util::IdentString};
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Field, Type};

use crate::{
    attrs::item::FieldAttributes,
    codegen::{
        ItemStatus, VersionDefinition,
        changes::{BTreeMapExt, ChangesetExt},
    },
    utils::FieldIdent,
};

pub struct VersionedField {
    pub original_attributes: Vec<Attribute>,
    pub changes: Option<BTreeMap<Version, ItemStatus>>,
    pub ident: FieldIdent,
    pub ty: Type,
}

impl VersionedField {
    pub fn new(field: Field, versions: &[VersionDefinition]) -> Result<Self> {
        let field_attributes = FieldAttributes::from_field(&field)?;
        field_attributes.validate_versions(versions)?;

        let field_ident = FieldIdent::from(
            field
                .ident
                .expect("internal error: field must have an ident"),
        );
        let changes = field_attributes
            .common
            .into_changeset(&field_ident, field.ty.clone());

        Ok(Self {
            original_attributes: field_attributes.attrs,
            ident: field_ident,
            ty: field.ty,
            changes,
        })
    }

    pub fn insert_container_versions(&mut self, versions: &[VersionDefinition]) {
        if let Some(changes) = &mut self.changes {
            changes.insert_container_versions(versions, &self.ty);
        }
    }

    pub fn generate_for_container(&self, version: &VersionDefinition) -> Option<TokenStream> {
        let original_attributes = &self.original_attributes;

        match &self.changes {
            Some(changes) => {
                // Check if the provided container version is present in the map
                // of actions. If it is, some action occurred in exactly that
                // version and thus code is generated for that field based on
                // the type of action.
                // If not, the provided version has no action attached to it.
                // The code generation then depends on the relation to other
                // versions (with actions).

                let field_type = &self.ty;

                // NOTE (@Techassi): https://rust-lang.github.io/rust-clippy/master/index.html#/expect_fun_call
                match changes.get(&version.inner).unwrap_or_else(|| {
                    panic!(
                        "internal error: chain must contain container version {}",
                        version.inner
                    )
                }) {
                    ItemStatus::Addition { ident, ty, .. } => Some(quote! {
                        #(#original_attributes)*
                        pub #ident: #ty,
                    }),
                    ItemStatus::Change {
                        to_ident, to_type, ..
                    } => Some(quote! {
                        #(#original_attributes)*
                        pub #to_ident: #to_type,
                    }),
                    ItemStatus::Deprecation {
                        ident: field_ident,
                        note,
                        ..
                    } => {
                        // FIXME (@Techassi): Emitting the deprecated attribute
                        // should cary over even when the item status is
                        // 'NoChange'.
                        // TODO (@Techassi): Make the generation of deprecated
                        // items customizable. When a container is used as a K8s
                        // CRD, the item must continue to exist, even when
                        // deprecated. For other versioning use-cases, that
                        // might not be the case.
                        let deprecated_attr = if let Some(note) = note {
                            quote! {#[deprecated = #note]}
                        } else {
                            quote! {#[deprecated]}
                        };

                        Some(quote! {
                            #(#original_attributes)*
                            #deprecated_attr
                            pub #field_ident: #field_type,
                        })
                    }
                    ItemStatus::NotPresent => None,
                    ItemStatus::NoChange {
                        previously_deprecated,
                        ident,
                        ty,
                        ..
                    } => {
                        // TODO (@Techassi): Also carry along the deprecation
                        // note.
                        let deprecated_attr = previously_deprecated.then(|| quote! {#[deprecated]});

                        Some(quote! {
                            #(#original_attributes)*
                            #deprecated_attr
                            pub #ident: #ty,
                        })
                    }
                }
            }
            None => {
                // If there is no chain of field actions, the field is not
                // versioned and therefore included in all versions.
                let field_ident = &self.ident;
                let field_type = &self.ty;

                Some(quote! {
                    #(#original_attributes)*
                    pub #field_ident: #field_type,
                })
            }
        }
    }

    // TODO (@Techassi): This should probably return an optional token stream.
    pub fn generate_for_upgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        from_struct_ident: &IdentString,
    ) -> TokenStream {
        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    // If both this status and the next one is NotPresent, which means
                    // a field was introduced after a bunch of versions, we don't
                    // need to generate any code for the From impl.
                    (ItemStatus::NotPresent, ItemStatus::NotPresent) => {
                        quote! {}
                    }
                    (
                        _,
                        ItemStatus::Addition {
                            ident, default_fn, ..
                        },
                    ) => quote! {
                        #ident: #default_fn(),
                    },
                    (
                        _,
                        ItemStatus::Change {
                            from_ident: old_field_ident,
                            upgrade_with,
                            to_ident,
                            ..
                        },
                    ) => match upgrade_with {
                        // The user specified a custom conversion function which
                        // will be used here instead of the default .into() call
                        // which utilizes From impls.
                        Some(upgrade_fn) => quote! {
                            #to_ident: #upgrade_fn(#from_struct_ident.#old_field_ident),
                        },
                        // Default .into() call using From impls.
                        None => quote! {
                            #to_ident: #from_struct_ident.#old_field_ident.into(),
                        },
                    },
                    (old, next) => {
                        let next_field_ident = next.get_ident();
                        let old_field_ident = old.get_ident();

                        // NOTE (@Techassi): Do we really need .into() here. I'm
                        // currently not sure why it is there and if it is needed
                        // in some edge cases.
                        quote! {
                            #next_field_ident: #from_struct_ident.#old_field_ident.into(),
                        }
                    }
                }
            }
            None => {
                let field_ident = &*self.ident;

                quote! {
                    #field_ident: #from_struct_ident.#field_ident.into(),
                }
            }
        }
    }

    pub fn generate_for_downgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: &VersionDefinition,
        from_struct_ident: &IdentString,
    ) -> TokenStream {
        match &self.changes {
            Some(changes) => {
                let next_change = changes.get_expect(&next_version.inner);
                let change = changes.get_expect(&version.inner);

                match (change, next_change) {
                    // If both this status and the next one is NotPresent, which means
                    // a field was introduced after a bunch of versions, we don't
                    // need to generate any code for the From impl.
                    (ItemStatus::NotPresent, ItemStatus::NotPresent) => {
                        quote! {}
                    }
                    (_, ItemStatus::Addition { .. }) => quote! {},
                    (
                        _,
                        ItemStatus::Change {
                            downgrade_with,
                            from_ident: old_field_ident,
                            to_ident,
                            ..
                        },
                    ) => match downgrade_with {
                        Some(downgrade_fn) => quote! {
                            #old_field_ident: #downgrade_fn(#from_struct_ident.#to_ident),
                        },
                        None => quote! {
                            #old_field_ident: #from_struct_ident.#to_ident.into(),
                        },
                    },
                    (old, next) => {
                        let next_field_ident = next.get_ident();
                        let old_field_ident = old.get_ident();

                        // NOTE (@Techassi): Do we really need .into() here. I'm
                        // currently not sure why it is there and if it is needed
                        // in some edge cases.
                        quote! {
                            #old_field_ident: #from_struct_ident.#next_field_ident.into(),
                        }
                    }
                }
            }
            None => {
                let field_ident = &*self.ident;

                quote! {
                    #field_ident: #from_struct_ident.#field_ident.into(),
                }
            }
        }
    }
}
