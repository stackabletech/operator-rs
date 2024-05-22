use std::collections::BTreeMap;

use darling::Error;
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, Ident};

use crate::{
    attrs::field::FieldAttributes,
    consts::DEPRECATED_PREFIX,
    gen::{neighbors::Neighbors, version::ContainerVersion, ToTokensExt},
};

/// A versioned field, which contains contains common [`Field`] data and a chain
/// of actions.
///
/// The chain of action maps versions to an action and the appropriate field
/// name. Additionally, the [`Field`] data can be used to forward attributes,
/// generate documention, etc.
#[derive(Debug)]
pub(crate) struct VersionedField {
    chain: Option<BTreeMap<Version, FieldStatus>>,
    inner: Field,
}

impl ToTokensExt<&ContainerVersion> for VersionedField {
    fn to_tokens(&self, container_version: &ContainerVersion) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => {
                // Check if the provided container version is present in the map
                // of actions. If it is, some action occured in exactly that
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
                    FieldStatus::Added(field_ident) => Some(quote! {
                        pub #field_ident: #field_type,
                    }),
                    FieldStatus::Renamed { _from: _, to } => Some(quote! {
                        pub #to: #field_type,
                    }),
                    FieldStatus::Deprecated {
                        ident: field_ident,
                        note,
                    } => Some(quote! {
                        #[deprecated = #note]
                        pub #field_ident: #field_type,
                    }),
                    FieldStatus::NotPresent => None,
                    FieldStatus::NoChange(field_ident) => Some(quote! {
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
}

impl VersionedField {
    /// Create a new versioned field by creating a status chain for each version
    /// defined in an action in the field attribute.
    ///
    /// This chain will get extended by the versions defined on the container by
    /// calling the [`VersionedField::insert_container_versions`] function.
    pub(crate) fn new(field: Field, attrs: FieldAttributes) -> Result<Self, Error> {
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
        if let Some(deprecated) = attrs.deprecated {
            let mut actions = BTreeMap::new();

            let ident = field.ident.as_ref().unwrap();
            actions.insert(
                *deprecated.since,
                FieldStatus::Deprecated {
                    ident: ident.clone(),
                    note: deprecated.note.to_string(),
                },
            );

            // When the field is deprecated, any rename which occured beforehand
            // requires access to the field ident to infer the field ident for
            // the latest rename.
            let mut ident = format_ident!(
                "{ident}",
                ident = ident.to_string().replace(DEPRECATED_PREFIX, "")
            );

            for rename in attrs.renames.iter().rev() {
                let from = format_ident!("{from}", from = *rename.from);
                actions.insert(
                    *rename.since,
                    FieldStatus::Renamed {
                        _from: from.clone(),
                        to: ident,
                    },
                );
                ident = from;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = attrs.added {
                actions.insert(*added.since, FieldStatus::Added(ident));
            }

            Ok(Self {
                chain: Some(actions),
                inner: field,
            })
        } else if !attrs.renames.is_empty() {
            let mut actions = BTreeMap::new();
            let mut ident = field.ident.clone().unwrap();

            for rename in attrs.renames.iter().rev() {
                let from = format_ident!("{from}", from = *rename.from);
                actions.insert(
                    *rename.since,
                    FieldStatus::Renamed {
                        _from: from.clone(),
                        to: ident,
                    },
                );
                ident = from;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = attrs.added {
                actions.insert(*added.since, FieldStatus::Added(ident));
            }

            dbg!(&actions);

            Ok(Self {
                chain: Some(actions),
                inner: field,
            })
        } else {
            if let Some(added) = attrs.added {
                let mut actions = BTreeMap::new();

                actions.insert(
                    *added.since,
                    FieldStatus::Added(field.ident.clone().unwrap()),
                );

                return Ok(Self {
                    chain: Some(actions),
                    inner: field,
                });
            }

            Ok(Self {
                chain: None,
                inner: field,
            })
        }
    }

    /// Inserts container versions not yet present in the status chain.
    ///
    /// When intially creating a new [`VersionedField`], the code doesn't have
    /// access to the versions defined on the container. This function inserts
    /// all non-present container versions and decides which status and ident
    /// is the right fit based on the status neighbors.
    ///
    /// This continous chain ensures that when generating code (tokens), each
    /// field can lookup the status for a requested version.
    pub(crate) fn insert_container_versions(&mut self, versions: &Vec<ContainerVersion>) {
        if let Some(chain) = &mut self.chain {
            for version in versions {
                if chain.contains_key(&version.inner) {
                    continue;
                }

                match chain.get_neighbors(&version.inner) {
                    (None, Some(_)) => chain.insert(version.inner, FieldStatus::NotPresent),
                    (Some(status), None) => {
                        let ident = match status {
                            FieldStatus::Added(ident) => ident,
                            FieldStatus::Renamed { _from: _, to } => to,
                            FieldStatus::Deprecated { ident, note: _ } => ident,
                            FieldStatus::NoChange(ident) => ident,
                            FieldStatus::NotPresent => unreachable!(),
                        };

                        chain.insert(version.inner, FieldStatus::NoChange(ident.clone()))
                    }
                    (Some(status), Some(_)) => {
                        let ident = match status {
                            FieldStatus::Added(ident) => ident,
                            FieldStatus::Renamed { _from: _, to } => to,
                            FieldStatus::NoChange(ident) => ident,
                            _ => unreachable!(),
                        };

                        chain.insert(version.inner, FieldStatus::NoChange(ident.clone()))
                    }
                    _ => unreachable!(),
                };
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum FieldStatus {
    Added(Ident),
    Renamed { _from: Ident, to: Ident },
    Deprecated { ident: Ident, note: String },
    NoChange(Ident),
    NotPresent,
}
