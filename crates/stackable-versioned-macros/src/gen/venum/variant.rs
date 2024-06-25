use std::{collections::BTreeMap, ops::Deref};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Variant;

use crate::{
    attrs::variant::VariantAttributes,
    gen::common::{
        remove_deprecated_variant_prefix, ContainerVersion, Item, ItemStatus, VersionChain,
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

impl Item<Variant, VariantAttributes> for VersionedVariant {
    fn new(variant: Variant, attributes: VariantAttributes) -> Self {
        // NOTE (@Techassi): This is straight up copied from the VersionedField
        // impl. As mentioned above, unify this.
        if let Some(deprecated) = attributes.deprecated {
            let deprecated_ident = &variant.ident;

            // When the field is deprecated, any rename which occurred beforehand
            // requires access to the field ident to infer the field ident for
            // the latest rename.
            let mut ident = remove_deprecated_variant_prefix(&deprecated_ident);
            let mut actions = BTreeMap::new();

            actions.insert(
                *deprecated.since,
                ItemStatus::Deprecated {
                    previous_ident: ident.clone(),
                    ident: deprecated_ident.clone(),
                    note: deprecated.note.to_string(),
                },
            );

            for rename in attributes.renames.iter().rev() {
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
            if let Some(added) = attributes.added {
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
        } else if !attributes.renames.is_empty() {
            let mut actions = BTreeMap::new();
            let mut ident = variant.ident.clone();

            for rename in attributes.renames.iter().rev() {
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
            if let Some(added) = attributes.added {
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
            if let Some(added) = attributes.added {
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

    fn generate_for_container(&self, container_version: &ContainerVersion) -> Option<TokenStream> {
        match &self.chain {
            Some(chain) => match chain
                .get(&container_version.inner)
                .expect("internal error: chain must contain container version")
            {
                ItemStatus::Added { ident, .. } => Some(quote! {
                    #ident,
                }),
                ItemStatus::Renamed { from, to } => todo!(),
                ItemStatus::Deprecated {
                    previous_ident,
                    ident,
                    note,
                } => todo!(),
                ItemStatus::NoChange(_) => todo!(),
                ItemStatus::NotPresent => todo!(),
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

    fn generate_for_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &syn::Ident,
    ) -> TokenStream {
        todo!()
    }

    fn get_ident(&self, version: &ContainerVersion) -> Option<&syn::Ident> {
        match &self.chain {
            Some(chain) => chain
                .get(&version.inner)
                .expect("internal error: chain must contain container version")
                .get_ident(),
            None => Some(&self.inner.ident),
        }
    }

    fn chain(&mut self) -> Option<&mut VersionChain> {
        self.chain.as_mut()
    }
}
