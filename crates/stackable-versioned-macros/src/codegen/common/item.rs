use std::{collections::BTreeMap, marker::PhantomData, ops::Deref};

use quote::format_ident;
use syn::{spanned::Spanned, Attribute, Ident, Path};

use crate::{
    attrs::common::{ContainerAttributes, ItemAttributes, ValidateVersions},
    codegen::{
        chain::Neighbors,
        common::{ContainerVersion, VersionChain},
    },
};

/// This trait describes versioned container items, fields and variants in a
/// common way.
///
/// Shared functionality is implemented in a single place. Code which cannot be
/// shared is implemented on the wrapping type, like [`VersionedField`][1].
///
/// [1]: crate::codegen::vstruct::field::VersionedField
pub(crate) trait Item<I, A>: Sized
where
    A: for<'i> TryFrom<&'i I> + Attributes,
    I: Named + Spanned,
{
    /// Creates a new versioned item (struct field or enum variant) by consuming
    /// the parsed [Field](syn::Field) or [Variant](syn::Variant) and validating
    /// the versions of field actions against versions attached on the container.
    fn new(item: I, container_attrs: &ContainerAttributes) -> syn::Result<Self>;

    /// Inserts container versions not yet present in the status chain.
    ///
    /// When initially creating a new versioned item, the code doesn't have
    /// access to the versions defined on the container. This function inserts
    /// all non-present container versions and decides which status and ident
    /// is the right fit based on the status neighbors.
    ///
    /// This continuous chain ensures that when generating code (tokens), each
    /// field can lookup the status (and ident) for a requested version.
    fn insert_container_versions(&mut self, versions: &[ContainerVersion]);

    /// Returns the ident of the item based on the provided container version.
    fn get_ident(&self, version: &ContainerVersion) -> Option<&Ident>;
}

/// This trait enables access to the ident of named items, like fields and
/// variants.
///
/// It additionally provides a function to retrieve the cleaned ident, which
/// removes the deprecation prefixes.
pub(crate) trait Named {
    fn cleaned_ident(&self) -> Ident;
    fn ident(&self) -> &Ident;
}

/// This trait enables access to the common and original attributes across field
/// and variant attributes.
pub(crate) trait Attributes {
    /// The common attributes defined by the versioned macro.
    fn common_attributes_owned(self) -> ItemAttributes;

    /// The common attributes defined by the versioned macro.
    fn common_attributes(&self) -> &ItemAttributes;

    /// The attributes applied to the item outside of the versioned macro.
    fn original_attributes(&self) -> &Vec<Attribute>;
}

/// This struct combines common code for versioned fields and variants.
///
/// Most of the initial creation of a versioned field and variant are identical.
/// Currently, the following steps are unified:
///
/// - Initial creation of the action chain based on item attributes.
/// - Insertion of container versions into the chain.
///
/// The generic type parameter `I` describes the type of the versioned item,
/// usually [`Field`](syn::Field) or [`Variant`](syn::Variant). The parameter
/// `A` indicates the type of item attributes, usually [`FieldAttributes`][1] or
/// [`VariantAttributes`][2] depending on the used item type. As this type is
/// only needed during creation of [`Self`](VersionedItem), we must use a
/// [`PhantomData`] marker.
///
/// [1]: crate::attrs::field::FieldAttributes
/// [2]: crate::attrs::variant::VariantAttributes
#[derive(Debug)]
pub(crate) struct VersionedItem<I, A>
where
    A: for<'i> TryFrom<&'i I> + Attributes,
    I: Named + Spanned,
{
    pub(crate) chain: Option<VersionChain>,
    pub(crate) inner: I,
    pub(crate) original_attributes: Vec<Attribute>,
    _marker: PhantomData<A>,
}

impl<I, A> Item<I, A> for VersionedItem<I, A>
where
    syn::Error: for<'i> From<<A as TryFrom<&'i I>>::Error>,
    A: for<'i> TryFrom<&'i I> + Attributes + ValidateVersions<I>,
    I: Named + Spanned,
{
    fn new(item: I, container_attrs: &ContainerAttributes) -> syn::Result<Self> {
        // We use the TryFrom trait here, because the type parameter `A` can use
        // it as a trait bound. Internally this then calls either `from_field`
        // for field attributes or `from_variant` for variant attributes. Sadly
        // darling doesn't provide a "generic" trait which abstracts over the
        // different `from_` functions.
        let attrs = A::try_from(&item)?;
        attrs.validate_versions(container_attrs, &item)?;

        // These are the attributes added to the item outside of the macro.
        let original_attributes = attrs.original_attributes().clone();

        // These are the versioned macro attrs that are common to all items.
        let common_attributes = attrs.common_attributes_owned();

        // Constructing the action chain requires going through the actions
        // starting at the end, because the container definition always
        // represents the latest (most up-to-date) version of that struct.
        // That's why the following code needs to go through the actions in
        // reverse order, as otherwise it is impossible to extract the item
        // ident for each version.

        // Deprecating an item is always the last state an item can end up in.
        // For items which are not deprecated, the last change is either the
        // latest rename or addition, which is handled below. The ident of the
        // deprecated item is guaranteed to include the 'deprecated_' or
        // 'DEPRECATED_' prefix. The ident can thus be used as is.
        if let Some(deprecated) = common_attributes.deprecated {
            let deprecated_ident = item.ident();

            // When the item is deprecated, any rename which occurred beforehand
            // requires access to the item ident to infer the item ident for
            // the latest rename.
            let mut ident = item.cleaned_ident();
            let mut actions = BTreeMap::new();

            actions.insert(
                *deprecated.since,
                ItemStatus::Deprecated {
                    previous_ident: ident.clone(),
                    ident: deprecated_ident.clone(),
                    note: deprecated.note.to_string(),
                },
            );

            for rename in common_attributes.renames.iter().rev() {
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
            if let Some(added) = common_attributes.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                    },
                );
            }

            Ok(Self {
                _marker: PhantomData,
                chain: Some(actions),
                inner: item,
                original_attributes,
            })
        } else if !common_attributes.renames.is_empty() {
            let mut actions = BTreeMap::new();
            let mut ident = item.ident().clone();

            for rename in common_attributes.renames.iter().rev() {
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
            if let Some(added) = common_attributes.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                    },
                );
            }

            Ok(Self {
                _marker: PhantomData,
                chain: Some(actions),
                inner: item,
                original_attributes,
            })
        } else {
            if let Some(added) = common_attributes.added {
                let mut actions = BTreeMap::new();

                actions.insert(
                    *added.since,
                    ItemStatus::Added {
                        default_fn: added.default_fn.deref().clone(),
                        ident: item.ident().clone(),
                    },
                );

                return Ok(Self {
                    _marker: PhantomData,
                    chain: Some(actions),
                    inner: item,
                    original_attributes,
                });
            }

            Ok(Self {
                _marker: PhantomData,
                chain: None,
                inner: item,
                original_attributes,
            })
        }
    }

    fn insert_container_versions(&mut self, versions: &[ContainerVersion]) {
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

    fn get_ident(&self, version: &ContainerVersion) -> Option<&Ident> {
        match &self.chain {
            Some(chain) => chain
                .get(&version.inner)
                .expect("internal error: chain must contain container version")
                .get_ident(),
            None => Some(self.inner.ident()),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ItemStatus {
    Added {
        ident: Ident,
        default_fn: Path,
    },
    Renamed {
        from: Ident,
        to: Ident,
    },
    Deprecated {
        previous_ident: Ident,
        ident: Ident,
        note: String,
    },
    NoChange(Ident),
    NotPresent,
}

impl ItemStatus {
    pub(crate) fn get_ident(&self) -> Option<&Ident> {
        match &self {
            ItemStatus::Added { ident, .. } => Some(ident),
            ItemStatus::Renamed { to, .. } => Some(to),
            ItemStatus::Deprecated { ident, .. } => Some(ident),
            ItemStatus::NoChange(ident) => Some(ident),
            ItemStatus::NotPresent => None,
        }
    }
}
