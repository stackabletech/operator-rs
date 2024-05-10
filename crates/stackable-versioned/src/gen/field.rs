use std::collections::BTreeMap;

use darling::Error;
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, Ident};

use crate::{
    attrs::field::FieldAttributes,
    consts::DEPRECATED_PREFIX,
    gen::{version::ContainerVersion, ToTokensExt},
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

impl ToTokensExt for VersionedField {
    fn to_tokens_for_version(&self, container_version: &ContainerVersion) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => {
                // Check if the provided container version is present in the map
                // of actions. If it is, some action occured in exactly that
                // version and thus code is generated for that field based on
                // the type of action.
                // If not, the provided version has no action attached to it.
                // The code generation then depends on the relation to other
                // versions (with actions).

                // TODO (@Techassi): Make this more robust by also including
                // the container versions in the action chain. I'm not happy
                // with the follwoing code at all. It serves as a good first
                // implementation to get something out of the door.
                match chain.get(&container_version.inner) {
                    Some(action) => match action {
                        FieldStatus::Added(field_ident) => {
                            let field_type = &self.inner.ty;

                            Some(quote! {
                                pub #field_ident: #field_type,
                            })
                        }
                        FieldStatus::Renamed { from: _, to } => {
                            let field_type = &self.inner.ty;

                            Some(quote! {
                                pub #to: #field_type,
                            })
                        }
                        FieldStatus::Deprecated(field_ident) => {
                            let field_type = &self.inner.ty;

                            Some(quote! {
                                #[deprecated]
                                pub #field_ident: #field_type,
                            })
                        }
                    },
                    None => {
                        // Generate field if the container version is not
                        // included in the action chain. First we check the
                        // earliest field action version.
                        if let Some((version, action)) = chain.first_key_value() {
                            if container_version.inner < *version {
                                match action {
                                    FieldStatus::Added(_) => return None,
                                    FieldStatus::Renamed { from, to: _ } => {
                                        let field_type = &self.inner.ty;

                                        return Some(quote! {
                                            pub #from: #field_type,
                                        });
                                    }
                                    FieldStatus::Deprecated(field_ident) => {
                                        let field_type = &self.inner.ty;

                                        return Some(quote! {
                                            pub #field_ident: #field_type,
                                        });
                                    }
                                }
                            }
                        }

                        // Check the container version against the latest
                        // field action version.
                        if let Some((version, action)) = chain.last_key_value() {
                            if container_version.inner > *version {
                                match action {
                                    FieldStatus::Added(field_ident) => {
                                        let field_type = &self.inner.ty;

                                        return Some(quote! {
                                            pub #field_ident: #field_type,
                                        });
                                    }
                                    FieldStatus::Renamed { from: _, to } => {
                                        let field_type = &self.inner.ty;

                                        return Some(quote! {
                                            pub #to: #field_type,
                                        });
                                    }
                                    FieldStatus::Deprecated(field_ident) => {
                                        let field_type = &self.inner.ty;

                                        return Some(quote! {
                                            #[deprecated]
                                            pub #field_ident: #field_type,
                                        });
                                    }
                                }
                            }
                        }

                        // TODO (@Techassi): Handle versions which are in between
                        // versions defined in field actions.
                        None
                    }
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
            actions.insert(*deprecated.since, FieldStatus::Deprecated(ident.clone()));

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
                        from: from.clone(),
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
                        from: from.clone(),
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
}

#[derive(Debug)]
pub(crate) enum FieldStatus {
    Added(Ident),
    Renamed { from: Ident, to: Ident },
    Deprecated(Ident),
}
