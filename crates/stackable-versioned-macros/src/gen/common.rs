use std::{collections::BTreeMap, ops::Deref};

use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use syn::{Field, Ident, Path};

use crate::attrs::container::ContainerAttributes;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    pub(crate) deprecated: bool,
    pub(crate) skip_from: bool,
    pub(crate) inner: Version,
    pub(crate) ident: Ident,
}

impl From<&ContainerAttributes> for Vec<ContainerVersion> {
    fn from(attributes: &ContainerAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| ContainerVersion {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: Ident::new(&v.name.to_string(), Span::call_site()),
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect()
    }
}

pub(crate) trait Container<D>
where
    Self: Sized + Deref<Target = VersionedContainer>,
{
    fn new(ident: Ident, data: D, attributes: ContainerAttributes) -> syn::Result<Self>;

    /// This generates the complete code for a single versioned container.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the container with the appropriate items (fields or variants)
    ///  Additionally, it generates `From` implementations, which enable
    /// conversion from an older to a newer version.
    fn generate_tokens(&self) -> TokenStream;
}

#[derive(Debug)]
pub(crate) struct VersionedContainer {
    pub(crate) ident: Ident,
    pub(crate) versions: Vec<ContainerVersion>,
    pub(crate) items: Vec<VersionedItem>,
    pub(crate) from_ident: Ident,
    pub(crate) skip_from: bool,
}

pub(crate) trait Item<A>
where
    Self: Sized,
{
    /// Create a new versioned item (field or variant) by creating a status
    /// chain for each version defined in an action in the item attribute.
    ///
    /// This chain will get extended by the versions defined on the container by
    /// calling the [`Item::insert_container_versions`] function.
    fn new(field: Field, attributes: A) -> Self;
    fn insert_container_versions(&mut self, versions: &[ContainerVersion]);
    fn generate_for_container(&self, container_version: &ContainerVersion) -> Option<TokenStream>;
    fn generate_for_from_impl(
        &self,
        version: &ContainerVersion,
        next_version: &ContainerVersion,
        from_ident: &Ident,
    ) -> TokenStream;
    fn get_ident(&self, version: &ContainerVersion) -> Option<&Ident>;
}

#[derive(Debug)]
pub(crate) struct VersionedItem {
    pub(crate) chain: Option<BTreeMap<Version, ItemStatus>>,
    pub(crate) inner: Field,
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
