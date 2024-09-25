use std::{collections::BTreeMap, marker::PhantomData, ops::Deref};

use quote::format_ident;
use syn::{spanned::Spanned, Attribute, Ident, Path, Type};

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
    I: InnerItem,
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

pub(crate) trait InnerItem: Named + Spanned {
    fn ty(&self) -> Type;
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
    I: InnerItem,
{
    pub(crate) original_attributes: Vec<Attribute>,
    pub(crate) chain: Option<VersionChain>,
    pub(crate) inner: I,
    _marker: PhantomData<A>,
}

impl<I, A> Item<I, A> for VersionedItem<I, A>
where
    syn::Error: for<'i> From<<A as TryFrom<&'i I>>::Error>,
    A: for<'i> TryFrom<&'i I> + Attributes + ValidateVersions<I>,
    I: InnerItem,
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
        // latest change or addition, which is handled below. The ident of the
        // deprecated item is guaranteed to include the 'deprecated_' or
        // 'DEPRECATED_' prefix. The ident can thus be used as is.
        if let Some(deprecated) = common_attributes.deprecated {
            let deprecated_ident = item.ident();

            // When the item is deprecated, any change which occurred beforehand
            // requires access to the item ident to infer the item ident for
            // the latest change.
            let mut ident = item.cleaned_ident();
            let mut ty = item.ty();

            let mut actions = BTreeMap::new();

            actions.insert(
                *deprecated.since,
                ItemStatus::Deprecation {
                    previous_ident: ident.clone(),
                    ident: deprecated_ident.clone(),
                    note: deprecated.note.as_deref().cloned(),
                },
            );

            for change in common_attributes.changes.iter().rev() {
                let from_ident = if let Some(from) = change.from_name.as_deref() {
                    format_ident!("{from}")
                } else {
                    ident.clone()
                };

                // TODO (@Techassi): This is an awful lot of cloning, can we get
                // rid of it?
                let from_ty = change
                    .from_type
                    .as_ref()
                    .map(|sv| sv.deref().clone())
                    .unwrap_or(ty.clone());

                actions.insert(
                    *change.since,
                    ItemStatus::Change {
                        from_ident: from_ident.clone(),
                        to_ident: ident,
                        from_type: from_ty.clone(),
                        to_type: ty,
                    },
                );

                ident = from_ident;
                ty = from_ty;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = common_attributes.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Addition {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                        ty,
                    },
                );
            }

            Ok(Self {
                _marker: PhantomData,
                chain: Some(actions),
                original_attributes,
                inner: item,
            })
        } else if !common_attributes.changes.is_empty() {
            let mut ident = item.ident().clone();
            let mut ty = item.ty();

            let mut actions = BTreeMap::new();

            for change in common_attributes.changes.iter().rev() {
                let from_ident = if let Some(from) = change.from_name.as_deref() {
                    format_ident!("{from}")
                } else {
                    ident.clone()
                };

                // TODO (@Techassi): This is an awful lot of cloning, can we get
                // rid of it?
                let from_ty = change
                    .from_type
                    .as_ref()
                    .map(|sv| sv.deref().clone())
                    .unwrap_or(ty.clone());

                actions.insert(
                    *change.since,
                    ItemStatus::Change {
                        from_ident: from_ident.clone(),
                        to_ident: ident,
                        from_type: from_ty.clone(),
                        to_type: ty,
                    },
                );

                ident = from_ident;
                ty = from_ty;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = common_attributes.added {
                actions.insert(
                    *added.since,
                    ItemStatus::Addition {
                        default_fn: added.default_fn.deref().clone(),
                        ident,
                        ty,
                    },
                );
            }

            Ok(Self {
                _marker: PhantomData,
                chain: Some(actions),
                original_attributes,
                inner: item,
            })
        } else {
            if let Some(added) = common_attributes.added {
                let mut actions = BTreeMap::new();

                actions.insert(
                    *added.since,
                    ItemStatus::Addition {
                        default_fn: added.default_fn.deref().clone(),
                        ident: item.ident().clone(),
                        ty: item.ty(),
                    },
                );

                return Ok(Self {
                    _marker: PhantomData,
                    chain: Some(actions),
                    original_attributes,
                    inner: item,
                });
            }

            Ok(Self {
                _marker: PhantomData,
                original_attributes,
                chain: None,
                inner: item,
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
                        ItemStatus::Addition { .. } => {
                            chain.insert(version.inner, ItemStatus::NotPresent)
                        }
                        ItemStatus::Change {
                            from_ident,
                            from_type,
                            ..
                        } => chain.insert(
                            version.inner,
                            ItemStatus::NoChange {
                                previously_deprecated: false,
                                ident: from_ident.clone(),
                                ty: from_type.clone(),
                            },
                        ),
                        ItemStatus::Deprecation { previous_ident, .. } => chain.insert(
                            version.inner,
                            ItemStatus::NoChange {
                                previously_deprecated: false,
                                ident: previous_ident.clone(),
                                ty: self.inner.ty(),
                            },
                        ),
                        ItemStatus::NoChange {
                            previously_deprecated,
                            ident,
                            ty,
                        } => chain.insert(
                            version.inner,
                            ItemStatus::NoChange {
                                previously_deprecated: *previously_deprecated,
                                ident: ident.clone(),
                                ty: ty.clone(),
                            },
                        ),
                        ItemStatus::NotPresent => unreachable!(),
                    },
                    (Some(status), None) => {
                        let (ident, ty, previously_deprecated) = match status {
                            ItemStatus::Addition { ident, ty, .. } => (ident, ty, false),
                            ItemStatus::Change {
                                to_ident, to_type, ..
                            } => (to_ident, to_type, false),
                            ItemStatus::Deprecation { ident, .. } => {
                                (ident, &self.inner.ty(), true)
                            }
                            ItemStatus::NoChange {
                                previously_deprecated,
                                ident,
                                ty,
                                ..
                            } => (ident, ty, *previously_deprecated),
                            ItemStatus::NotPresent => unreachable!(),
                        };

                        chain.insert(
                            version.inner,
                            ItemStatus::NoChange {
                                previously_deprecated,
                                ident: ident.clone(),
                                ty: ty.clone(),
                            },
                        )
                    }
                    (Some(status), Some(_)) => {
                        let (ident, ty, previously_deprecated) = match status {
                            ItemStatus::Addition { ident, ty, .. } => (ident, ty, false),
                            ItemStatus::Change {
                                to_ident, to_type, ..
                            } => (to_ident, to_type, false),
                            ItemStatus::NoChange {
                                previously_deprecated,
                                ident,
                                ty,
                                ..
                            } => (ident, ty, *previously_deprecated),
                            _ => unreachable!(),
                        };

                        chain.insert(
                            version.inner,
                            ItemStatus::NoChange {
                                previously_deprecated,
                                ident: ident.clone(),
                                ty: ty.clone(),
                            },
                        )
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

#[derive(Debug, PartialEq)]
pub(crate) enum ItemStatus {
    Addition {
        ident: Ident,
        default_fn: Path,
        // NOTE (@Techassi): We need to carry idents and type information in
        // nearly every status. Ideally, we would store this in separate maps.
        ty: Type,
    },
    Change {
        from_ident: Ident,
        to_ident: Ident,
        from_type: Type,
        to_type: Type,
    },
    Deprecation {
        previous_ident: Ident,
        note: Option<String>,
        ident: Ident,
    },
    NoChange {
        previously_deprecated: bool,
        ident: Ident,
        ty: Type,
    },
    NotPresent,
}

impl ItemStatus {
    pub(crate) fn get_ident(&self) -> Option<&Ident> {
        match &self {
            ItemStatus::Addition { ident, .. } => Some(ident),
            ItemStatus::Change { to_ident, .. } => Some(to_ident),
            ItemStatus::Deprecation { ident, .. } => Some(ident),
            ItemStatus::NoChange { ident, .. } => Some(ident),
            ItemStatus::NotPresent => None,
        }
    }
}
