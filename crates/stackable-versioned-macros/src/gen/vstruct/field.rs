use std::{collections::BTreeMap, ops::Deref};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, Ident};

use crate::{
    attrs::field::FieldAttributes,
    gen::common::{
        remove_deprecated_field_prefix, ContainerVersion, Item, ItemStatus, VersionChain,
    },
};

/// A versioned field, which contains contains common [`Field`] data and a chain
/// of actions.
///
/// The chain of action maps versions to an action and the appropriate field
/// name. Additionally, the [`Field`] data can be used to forward attributes,
/// generate documentation, etc.
#[derive(Debug)]
pub(crate) struct VersionedField {
    pub(crate) chain: Option<VersionChain>,
    pub(crate) inner: Field,
}

impl Item<syn::Field, FieldAttributes> for VersionedField {
    fn new(field: syn::Field, field_attrs: FieldAttributes) -> Self {
        // Constructing the action chain requires going through the actions from
        // the end, because the base struct always represents the latest (most
        // up-to-date) version of that struct. That's why the following code
        // needs to go through the actions in reverse order, as otherwise it is
        // impossible to extract the field ident for each version.

        // Deprecating a field is always the last state a field can end up in. For
        // fields which are not deprecated, the last change is either the latest
        // rename or addition, which is handled below.
        // The ident of the deprecated field is guaranteed to include the
        // 'deprecated_' prefix. The ident can thus be used as is.
        if let Some(deprecated) = field_attrs.common.deprecated {
            let deprecated_ident = field
                .ident
                .as_ref()
                .expect("internal error: field must have an ident");

            // When the field is deprecated, any rename which occurred beforehand
            // requires access to the field ident to infer the field ident for
            // the latest rename.
            let mut ident = remove_deprecated_field_prefix(&deprecated_ident);
            let mut actions = BTreeMap::new();

            actions.insert(
                *deprecated.since,
                ItemStatus::Deprecated {
                    previous_ident: ident.clone(),
                    ident: deprecated_ident.clone(),
                    note: deprecated.note.to_string(),
                },
            );

            for rename in field_attrs.common.renames.iter().rev() {
                let from = format_ident!("{from}", from = *rename.from);
                actions.insert(
                    *rename.since,
                    ItemStatus::Renamed {
                        from: from.clone(),
                        to: ident,
                    },
                );
                ident = from;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = field_attrs.common.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                    },
                );
            }

            Self {
                chain: Some(actions),
                inner: field,
            }
        } else if !field_attrs.common.renames.is_empty() {
            let mut actions = BTreeMap::new();
            let mut ident = field
                .ident
                .clone()
                .expect("internal error: field must have an ident");

            for rename in field_attrs.common.renames.iter().rev() {
                let from = format_ident!("{from}", from = *rename.from);
                actions.insert(
                    *rename.since,
                    ItemStatus::Renamed {
                        from: from.clone(),
                        to: ident,
                    },
                );
                ident = from;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = field_attrs.common.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                    },
                );
            }

            Self {
                chain: Some(actions),
                inner: field,
            }
        } else {
            if let Some(added) = field_attrs.common.added {
                let mut actions = BTreeMap::new();

                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident: field
                            .ident
                            .clone()
                            .expect("internal error: field must have a name"),
                    },
                );

                return Self {
                    chain: Some(actions),
                    inner: field,
                };
            }

            Self {
                chain: None,
                inner: field,
            }
        }
    }

    fn generate_for_container(&self, container_version: &ContainerVersion) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => {
                // Check if the provided container version is present in the map
                // of actions. If it is, some action occurred in exactly that
                // version and thus code is generated for that field based on
                // the type of action.
                // If not, the provided version has no action attached to it.
                // The code generation then depends on the relation to other
                // versions (with actions).

                let field_type = &self.inner.ty;

                match chain
                    .get(&container_version.inner)
                    .expect("internal error: chain must contain container version")
                {
                    ItemStatus::Added { ident, .. } => Some(quote! {
                        pub #ident: #field_type,
                    }),
                    ItemStatus::Renamed { to, .. } => Some(quote! {
                        pub #to: #field_type,
                    }),
                    ItemStatus::Deprecated {
                        ident: field_ident,
                        note,
                        ..
                    } => Some(quote! {
                        #[deprecated = #note]
                        pub #field_ident: #field_type,
                    }),
                    ItemStatus::NotPresent => None,
                    ItemStatus::NoChange(field_ident) => Some(quote! {
                        pub #field_ident: #field_type,
                    }),
                }
            }
            None => {
                // If there is no chain of field actions, the field is not
                // versioned and code generation is straight forward.
                // Unversioned fields are always included in versioned structs.
                let field_ident = &self.inner.ident;
                let field_type = &self.inner.ty;

                Some(quote! {
                    pub #field_ident: #field_type,
                })
            }
        }
    }

    fn generate_for_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
    ) -> TokenStream {
        match &self.chain {
            Some(chain) => {
                match (
                    chain
                        .get(&version.inner)
                        .expect("internal error: chain must contain container version"),
                    chain
                        .get(&next_version.inner)
                        .expect("internal error: chain must contain container version"),
                ) {
                    (_, ItemStatus::Added { ident, default_fn }) => quote! {
                        #ident: #default_fn(),
                    },
                    (old, next) => {
                        let old_field_ident = old
                            .get_ident()
                            .expect("internal error: old field must have a name");

                        let next_field_ident = next
                            .get_ident()
                            .expect("internal error: new field must have a name");

                        quote! {
                            #next_field_ident: #from_ident.#old_field_ident,
                        }
                    }
                }
            }
            None => {
                let field_ident = &self.inner.ident;
                quote! {
                    #field_ident: #from_ident.#field_ident,
                }
            }
        }
    }

    fn get_ident(&self, version: &ContainerVersion) -> Option<&Ident> {
        match &self.chain {
            Some(chain) => chain
                .get(&version.inner)
                .expect("internal error: chain must contain container version")
                .get_ident(),
            None => self.inner.ident.as_ref(),
        }
    }

    fn chain(&mut self) -> Option<&mut VersionChain> {
        self.chain.as_mut()
    }
}
