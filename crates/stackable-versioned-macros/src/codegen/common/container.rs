use std::ops::Deref;

use convert_case::{Case, Casing};
use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{Attribute, Ident, Visibility};

use crate::{
    attrs::common::StandaloneContainerAttributes,
    codegen::common::VersionDefinition,
    consts::{DEPRECATED_FIELD_PREFIX, DEPRECATED_VARIANT_PREFIX},
};

/// This trait helps to unify versioned containers, like structs and enums.
///
/// This trait is implemented by wrapper structs, which wrap the generic
/// [`VersionedContainer`] struct. The generic type parameter `D` describes the
/// kind of data, like [`DataStruct`](syn::DataStruct) in case of a struct and
/// [`DataEnum`](syn::DataEnum) in case of an enum.
/// The type parameter `I` describes the type of the versioned items, like
/// [`VersionedField`][1] and [`VersionedVariant`][2].
///
/// [1]: crate::codegen::vstruct::field::VersionedField
/// [2]: crate::codegen::venum::variant::VersionedVariant
pub(crate) trait Container<D, I>
where
    Self: Sized + Deref<Target = VersionedContainer<I>>,
{
    /// Creates a new versioned container.
    fn new(
        input: ContainerInput,
        data: D,
        attributes: StandaloneContainerAttributes,
    ) -> syn::Result<Self>;

    /// This generates the complete code for a single versioned container.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the container with the appropriate items (fields or variants)
    /// Additionally, it generates `From` implementations, which enable
    /// conversion from an older to a newer version.
    fn generate_standalone_tokens(&self) -> TokenStream;

    fn generate_nested_tokens(&self) -> TokenStream;
}

/// Provides extra functionality on top of [`struct@Ident`]s used to name containers.
pub(crate) trait ContainerIdentExt {
    /// Removes the 'Spec' suffix from the [`struct@Ident`].
    fn as_cleaned_kubernetes_ident(&self) -> IdentString;

    /// Transforms the [`struct@Ident`] into one usable in the [`From`] impl.
    fn as_from_impl_ident(&self) -> IdentString;
}

impl ContainerIdentExt for Ident {
    fn as_cleaned_kubernetes_ident(&self) -> IdentString {
        let ident = format_ident!("{}", self.to_string().trim_end_matches("Spec"));
        IdentString::new(ident)
    }

    fn as_from_impl_ident(&self) -> IdentString {
        let ident = format_ident!("__sv_{}", self.to_string().to_lowercase());
        IdentString::new(ident)
    }
}

/// Provides extra functionality on top of [`struct@Ident`]s used to name items, like fields and
/// variants.
pub(crate) trait ItemIdentExt {
    /// Removes deprecation prefixed from field or variant idents.
    fn as_cleaned_ident(&self) -> IdentString;
}

impl ItemIdentExt for Ident {
    fn as_cleaned_ident(&self) -> IdentString {
        let ident = self.to_string();
        let ident = ident
            .trim_start_matches(DEPRECATED_FIELD_PREFIX)
            .trim_start_matches(DEPRECATED_VARIANT_PREFIX)
            .trim_start_matches('_');

        IdentString::new(format_ident!("{ident}"))
    }
}

pub(crate) trait VersionExt {
    fn as_variant_ident(&self) -> Ident;
}

impl VersionExt for Version {
    fn as_variant_ident(&self) -> Ident {
        format_ident!("{ident}", ident = self.to_string().to_case(Case::Pascal))
    }
}

/// This struct bundles values from [`DeriveInput`][1].
///
/// [`DeriveInput`][1] cannot be used directly when constructing a
/// [`VersionedStruct`][2] or [`VersionedEnum`][3] because we run into borrow
/// issues caused by the match statement which extracts the data.
///
/// [1]: syn::DeriveInput
/// [2]: crate::codegen::vstruct::VersionedStruct
/// [3]: crate::codegen::venum::VersionedEnum
pub(crate) struct ContainerInput {
    pub(crate) original_attributes: Vec<Attribute>,
    pub(crate) visibility: Visibility,
    pub(crate) ident: Ident,
}

/// Stores individual versions of a single container.
///
/// Each version tracks item actions, which describe if the item was added,
/// renamed or deprecated in that particular version. Items which are not
/// versioned are included in every version of the container.
#[derive(Debug)]
pub(crate) struct VersionedContainer<I> {
    /// List of declared versions for this container. Each version generates a
    /// definition with appropriate items.
    pub(crate) versions: Vec<VersionDefinition>,

    /// The original attributes that were added to the container.
    pub(crate) original_attributes: Vec<Attribute>,

    /// The visibility of the versioned container. Used to forward the
    /// visibility during code generation.
    pub(crate) visibility: Visibility,

    /// List of items defined in the original container. How, and if, an item
    /// should generate code, is decided by the currently generated version.
    pub(crate) items: Vec<I>,

    /// Different options which influence code generation.
    pub(crate) options: VersionedContainerOptions,

    /// A collection of container idents used for different purposes.
    pub(crate) idents: VersionedContainerIdents,
}

impl<I> VersionedContainer<I> {
    /// Creates a new versioned Container which contains common data shared
    /// across structs and enums.
    pub(crate) fn new(
        input: ContainerInput,
        attributes: StandaloneContainerAttributes,
        versions: Vec<VersionDefinition>,
        items: Vec<I>,
    ) -> Self {
        let ContainerInput {
            original_attributes,
            visibility,
            ident,
        } = input;

        let skip_from = attributes
            .common_option_args
            .skip
            .map_or(false, |s| s.from.is_present());

        let kubernetes_options = attributes.kubernetes_args.map(|a| KubernetesOptions {
            skip_merged_crd: a.skip.map_or(false, |s| s.merged_crd.is_present()),
            namespaced: a.namespaced.is_present(),
            singular: a.singular,
            plural: a.plural,
            group: a.group,
            kind: a.kind,
        });

        let options = VersionedContainerOptions {
            kubernetes_options,
            skip_from,
        };

        let idents = VersionedContainerIdents {
            kubernetes: ident.as_cleaned_kubernetes_ident(),
            from: ident.as_from_impl_ident(),
            original: ident.into(),
        };

        VersionedContainer {
            original_attributes,
            visibility,
            versions,
            options,
            idents,
            items,
        }
    }
}

/// A collection of container idents used for different purposes.
#[derive(Debug)]
pub(crate) struct VersionedContainerIdents {
    /// The ident used in the context of Kubernetes specific code. This ident
    /// removes the 'Spec' suffix present in the definition container.
    pub(crate) kubernetes: IdentString,

    /// The original ident, or name, of the versioned container.
    pub(crate) original: IdentString,

    /// The ident used in the [`From`] impl.
    pub(crate) from: IdentString,
}

#[derive(Debug)]
pub(crate) struct VersionedContainerOptions {
    pub(crate) kubernetes_options: Option<KubernetesOptions>,
    pub(crate) skip_from: bool,
}

#[derive(Debug)]
pub(crate) struct KubernetesOptions {
    pub(crate) singular: Option<String>,
    pub(crate) plural: Option<String>,
    pub(crate) skip_merged_crd: bool,
    pub(crate) kind: Option<String>,
    pub(crate) namespaced: bool,
    pub(crate) group: String,
}
