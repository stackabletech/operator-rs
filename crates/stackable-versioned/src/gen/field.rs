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
        // Constructing the change chain requires going through the actions from
        // the end, because the base struct allways represents the latest (most
        // up-to-date) version of that struct. That's why the following code
        // needs to go through the changes in reverse order, as otherwise it is
        // impossible to extract the field ident for each version.

        // Deprecating a field is always the last status a field can up in. For
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
            let mut ident = format_ident!("{}", ident.to_string().replace(DEPRECATED_PREFIX, ""));

            for rename in attrs.renames.iter().rev() {
                let from = format_ident!("{}", *rename.from);
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
                let from = format_ident!("{}", *rename.from);
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

    /// Extend the already recorded actions with actions based on global
    /// container versions to construct a complete chain of actions for each
    /// field.
    pub(crate) fn _extend_with_container_versions(&mut self, _versions: &[ContainerVersion]) {
        // When creating this type via the new function, only directly attached
        // action can be inserted into the chain of actions. It doesn't contain
        // any actions based on the container versions. A quick example:
        //
        // Let's assume we have the following declared versions: v1, v2, v3, v4.
        // One field, let's call it foo, has two actions attached: added in v2
        // and deprecated in v3. So initially, the chain of actions only contains
        // two actions: added(v2) and deprecated(v3). But what happened to the
        // field in v1 and v4? This information can only be included in the
        // chain by looking at the container versions. In this particular
        // example the field wasn't present in v1 and isn't present from v4 and
        // onward. This action (or state) needs to be included in the chain of
        // actions. The resulting chain now looks like: not-present(v1),
        // added(v2), deprecated(v3), not-present(v4).
    }
}

#[derive(Debug)]
pub(crate) enum FieldStatus {
    Added(Ident),
    Renamed { from: Ident, to: Ident },
    Deprecated(Ident),
}
