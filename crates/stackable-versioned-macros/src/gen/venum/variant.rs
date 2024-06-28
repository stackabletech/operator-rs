use std::{collections::BTreeMap, ops::Deref};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Variant};

use crate::{
    attrs::variant::VariantAttributes,
    gen::{
        chain::{BTreeMapExt, Neighbors},
        common::{remove_deprecated_variant_prefix, ContainerVersion, ItemStatus, VersionChain},
    },
};

#[derive(Debug)]
pub(crate) struct VersionedVariant {
    chain: Option<VersionChain>,
    inner: Variant,
}

// TODO (@Techassi): Figure out a way to be able to only write the following code
// once for both a versioned field and variant, because the are practically
// identical.

impl VersionedVariant {
    pub(crate) fn new(variant: Variant, variant_attrs: VariantAttributes) -> Self {
        // NOTE (@Techassi): This is straight up copied from the VersionedField
        // impl. As mentioned above, unify this.
        if let Some(deprecated) = variant_attrs.common.deprecated {
            let deprecated_ident = &variant.ident;

            // When the field is deprecated, any rename which occurred beforehand
            // requires access to the field ident to infer the field ident for
            // the latest rename.
            let mut ident = remove_deprecated_variant_prefix(deprecated_ident);
            let mut actions = BTreeMap::new();

            actions.insert(
                *deprecated.since,
                ItemStatus::Deprecated {
                    previous_ident: ident.clone(),
                    ident: deprecated_ident.clone(),
                    note: deprecated.note.to_string(),
                },
            );

            for rename in variant_attrs.common.renames.iter().rev() {
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
            if let Some(added) = variant_attrs.common.added {
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
                inner: variant,
            }
        } else if !variant_attrs.common.renames.is_empty() {
            let mut actions = BTreeMap::new();
            let mut ident = variant.ident.clone();

            for rename in variant_attrs.common.renames.iter().rev() {
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
            if let Some(added) = variant_attrs.common.added {
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
                inner: variant,
            }
        } else {
            if let Some(added) = variant_attrs.common.added {
                let mut actions = BTreeMap::new();

                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident: variant.ident.clone(),
                    },
                );

                return Self {
                    chain: Some(actions),
                    inner: variant,
                };
            }

            Self {
                chain: None,
                inner: variant,
            }
        }
    }

    /// Inserts container versions not yet present in the status chain.
    ///
    /// When initially creating a new versioned item, the code doesn't have
    /// access to the versions defined on the container. This function inserts
    /// all non-present container versions and decides which status and ident
    /// is the right fit based on the status neighbors.
    ///
    /// This continuous chain ensures that when generating code (tokens), each
    /// field can lookup the status (and ident) for a requested version.
    pub(crate) fn insert_container_versions(&mut self, versions: &[ContainerVersion]) {
        if let Some(chain) = &mut self.chain {
            for version in versions {
                if chain.contains_key(&version.inner) {
                    continue;
                }

                match chain.get_neighbors(&version.inner) {
                    (None, Some(status)) => match status {
                        ItemStatus::Added { .. } => {
                            chain.insert(version.inner, ItemStatus::NotPresent)
                        }
                        ItemStatus::Renamed { from, .. } => {
                            chain.insert(version.inner, ItemStatus::NoChange(from.clone()))
                        }
                        ItemStatus::Deprecated { previous_ident, .. } => chain
                            .insert(version.inner, ItemStatus::NoChange(previous_ident.clone())),
                        ItemStatus::NoChange(ident) => {
                            chain.insert(version.inner, ItemStatus::NoChange(ident.clone()))
                        }
                        ItemStatus::NotPresent => unreachable!(),
                    },
                    (Some(status), None) => {
                        let ident = match status {
                            ItemStatus::Added { ident, .. } => ident,
                            ItemStatus::Renamed { to, .. } => to,
                            ItemStatus::Deprecated { ident, .. } => ident,
                            ItemStatus::NoChange(ident) => ident,
                            ItemStatus::NotPresent => unreachable!(),
                        };

                        chain.insert(version.inner, ItemStatus::NoChange(ident.clone()))
                    }
                    (Some(status), Some(_)) => {
                        let ident = match status {
                            ItemStatus::Added { ident, .. } => ident,
                            ItemStatus::Renamed { to, .. } => to,
                            ItemStatus::NoChange(ident) => ident,
                            _ => unreachable!(),
                        };

                        chain.insert(version.inner, ItemStatus::NoChange(ident.clone()))
                    }
                    _ => unreachable!(),
                };
            }
        }
    }

    pub(crate) fn generate_for_container(
        &self,
        container_version: &ContainerVersion,
    ) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => match chain
                .get(&container_version.inner)
                .expect("internal error: chain must contain container version")
            {
                ItemStatus::Added { ident, .. } => Some(quote! {
                    #ident,
                }),
                ItemStatus::Renamed { to, .. } => Some(quote! {
                    #to,
                }),
                ItemStatus::Deprecated { ident, .. } => Some(quote! {
                    #[deprecated]
                    #ident,
                }),
                ItemStatus::NoChange(ident) => Some(quote! {
                    #ident,
                }),
                ItemStatus::NotPresent => None,
            },
            None => {
                // If there is no chain of variant actions, the variant is not
                // versioned and code generation is straight forward.
                // Unversioned variants are always included in versioned enums.
                let variant_ident = &self.inner.ident;

                Some(quote! {
                    #variant_ident,
                })
            }
        }
    }

    pub(crate) fn generate_for_from_impl(
        &self,
        module_name: &Ident,
        next_module_name: &Ident,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        enum_ident: &Ident,
    ) -> TokenStream {
        match &self.chain {
            Some(chain) => match (
                chain.get_expect(&version.inner),
                chain.get_expect(&next_version.inner),
            ) {
                (_, ItemStatus::Added { .. }) => quote! {},
                (old, next) => {
                    let old_variant_ident = old
                        .get_ident()
                        .expect("internal error: old variant must have a name");
                    let next_variant_ident = next
                        .get_ident()
                        .expect("internal error: next variant must have a name");

                    quote! {
                        #module_name::#enum_ident::#old_variant_ident => #next_module_name::#enum_ident::#next_variant_ident,
                    }
                }
            },
            None => {
                let variant_ident = &self.inner.ident;

                quote! {
                    #module_name::#enum_ident::#variant_ident => #next_module_name::#enum_ident::#variant_ident,
                }
            }
        }
    }

    pub(crate) fn get_ident(&self, version: &ContainerVersion) -> Option<&syn::Ident> {
        match &self.chain {
            Some(chain) => chain
                .get(&version.inner)
                .expect("internal error: chain must contain container version")
                .get_ident(),
            None => Some(&self.inner.ident),
        }
    }
}
