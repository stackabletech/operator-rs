use darling::{Result, util::IdentString};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Attribute, Ident, ItemEnum, ItemStruct, Visibility};

use crate::{
    attrs::container::{StandaloneContainerAttributes, k8s::KubernetesArguments},
    codegen::{
        VersionDefinition,
        container::{r#enum::Enum, r#struct::Struct},
    },
    utils::ContainerIdentExt,
};

mod r#enum;
mod r#struct;

/// Contains common container data shared between structs and enums.
pub struct CommonContainerData {
    /// Original attributes placed on the container, like `#[derive()]` or `#[cfg()]`.
    pub original_attributes: Vec<Attribute>,

    /// Different options which influence code generation.
    pub options: ContainerOptions,

    /// A collection of container idents used for different purposes.
    pub idents: ContainerIdents,
}

/// Supported types of containers, structs and enums.
///
/// Abstracting away with kind of container is generated makes it possible to create a list of
/// containers when the macro is used on modules. This enum provides functions to generate code
/// which then internally call the appropriate function based on the variant.
pub enum Container {
    Struct(Struct),
    Enum(Enum),
}

impl Container {
    /// Generates the container definition for the specified `version`.
    pub fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        match self {
            Container::Struct(s) => s.generate_definition(version),
            Container::Enum(e) => e.generate_definition(version),
        }
    }

    /// Generates the `From<Version> for NextVersion` implementation for the container.
    pub fn generate_upgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => {
                s.generate_upgrade_from_impl(version, next_version, add_attributes)
            }
            Container::Enum(e) => {
                e.generate_upgrade_from_impl(version, next_version, add_attributes)
            }
        }
    }

    /// Generates the `From<NextVersion> for Version` implementation for the container.
    pub fn generate_downgrade_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => {
                s.generate_downgrade_from_impl(version, next_version, add_attributes)
            }
            Container::Enum(e) => {
                e.generate_downgrade_from_impl(version, next_version, add_attributes)
            }
        }
    }

    /// Generates Kubernetes specific code snippets.
    ///
    /// This function returns three values:
    ///
    /// - an enum variant ident,
    /// - an enum variant display string,
    /// - and a `CustomResource::crd()` call
    ///
    /// This function only returns `Some` if it is a struct. Enums cannot be used to define
    /// Kubernetes custom resources.
    pub fn generate_kubernetes_item(
        &self,
        version: &VersionDefinition,
    ) -> Option<(IdentString, String, TokenStream)> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_item(version),
            Container::Enum(_) => None,
        }
    }

    /// Generates Kubernetes specific code to merge two or more CRDs into one.
    ///
    /// This function only returns `Some` if it is a struct. Enums cannot be used to define
    /// Kubernetes custom resources.
    pub fn generate_kubernetes_merge_crds(
        &self,
        enum_variant_idents: &[IdentString],
        enum_variant_strings: &[String],
        fn_calls: &[TokenStream],
        vis: &Visibility,
        is_nested: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_merge_crds(
                enum_variant_idents,
                enum_variant_strings,
                fn_calls,
                vis,
                is_nested,
            ),
            Container::Enum(_) => None,
        }
    }

    pub fn generate_kubernetes_status_struct(&self) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_status_struct(),
            Container::Enum(_) => None,
        }
    }

    pub fn get_original_ident(&self) -> &Ident {
        match &self {
            Container::Struct(s) => s.common.idents.original.as_ident(),
            Container::Enum(e) => e.common.idents.original.as_ident(),
        }
    }
}

/// A versioned standalone container.
///
/// A standalone container is a container defined outside of a versioned module. See [`Module`][1]
/// for more information about versioned modules.
///
/// [1]: crate::codegen::module::Module
pub struct StandaloneContainer {
    versions: Vec<VersionDefinition>,
    container: Container,
    vis: Visibility,
}

impl StandaloneContainer {
    /// Creates a new versioned standalone struct.
    pub fn new_struct(
        item_struct: ItemStruct,
        attributes: StandaloneContainerAttributes,
    ) -> Result<Self> {
        let versions: Vec<_> = (&attributes).into();
        let vis = item_struct.vis.clone();

        let container = Container::new_standalone_struct(item_struct, attributes, &versions)?;

        Ok(Self {
            container,
            versions,
            vis,
        })
    }

    /// Creates a new versioned standalone enum.
    pub fn new_enum(
        item_enum: ItemEnum,
        attributes: StandaloneContainerAttributes,
    ) -> Result<Self> {
        let versions: Vec<_> = (&attributes).into();
        let vis = item_enum.vis.clone();

        let container = Container::new_standalone_enum(item_enum, attributes, &versions)?;

        Ok(Self {
            container,
            versions,
            vis,
        })
    }

    /// Generate tokens containing every piece of code required for a standalone container.
    pub fn generate_tokens(&self) -> TokenStream {
        let vis = &self.vis;

        let mut tokens = TokenStream::new();

        let mut kubernetes_merge_crds_fn_calls = Vec::new();
        let mut kubernetes_enum_variant_idents = Vec::new();
        let mut kubernetes_enum_variant_strings = Vec::new();

        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let container_definition = self.container.generate_definition(version);

            // NOTE (@Techassi): Using '.copied()' here does not copy or clone the data, but instead
            // removes one level of indirection of the double reference '&&'.
            let next_version = versions.peek().copied();

            // Generate the From impl for upgrading the CRD.
            let upgrade_from_impl =
                self.container
                    .generate_upgrade_from_impl(version, next_version, false);

            // Generate the From impl for downgrading the CRD.
            let downgrade_from_impl =
                self.container
                    .generate_downgrade_from_impl(version, next_version, false);

            // Add the #[deprecated] attribute when the version is marked as deprecated.
            let deprecated_attribute = version
                .deprecated
                .as_ref()
                .map(|note| quote! { #[deprecated = #note] });

            // Generate Kubernetes specific code which is placed outside of the container
            // definition.
            if let Some((enum_variant_ident, enum_variant_string, fn_call)) =
                self.container.generate_kubernetes_item(version)
            {
                kubernetes_merge_crds_fn_calls.push(fn_call);
                kubernetes_enum_variant_idents.push(enum_variant_ident);
                kubernetes_enum_variant_strings.push(enum_variant_string);
            }

            let version_ident = &version.ident;

            tokens.extend(quote! {
                #[automatically_derived]
                #deprecated_attribute
                #vis mod #version_ident {
                    use super::*;
                    #container_definition
                }

                #upgrade_from_impl
                #downgrade_from_impl
            });
        }

        tokens.extend(self.container.generate_kubernetes_merge_crds(
            &kubernetes_enum_variant_idents,
            &kubernetes_enum_variant_strings,
            &kubernetes_merge_crds_fn_calls,
            vis,
            false,
        ));

        tokens.extend(self.container.generate_kubernetes_status_struct());

        tokens
    }
}

/// A collection of container idents used for different purposes.
#[derive(Debug)]
pub struct ContainerIdents {
    /// The ident used in the context of Kubernetes specific code. This ident
    /// removes the 'Spec' suffix present in the definition container.
    pub kubernetes: IdentString,

    /// The original ident, or name, of the versioned container.
    pub original: IdentString,

    /// The ident used in the [`From`] impl.
    pub from: IdentString,
}

impl ContainerIdents {
    pub fn from(ident: Ident, kubernetes_arguments: Option<&KubernetesArguments>) -> Self {
        let kubernetes = kubernetes_arguments.map_or_else(
            || ident.as_cleaned_kubernetes_ident(),
            |options| {
                options.kind.as_ref().map_or_else(
                    || ident.as_cleaned_kubernetes_ident(),
                    |kind| IdentString::from(Ident::new(kind, Span::call_site())),
                )
            },
        );

        Self {
            from: ident.as_from_impl_ident(),
            original: ident.into(),
            kubernetes,
        }
    }
}

#[derive(Debug)]
pub struct ContainerOptions {
    pub kubernetes_arguments: Option<KubernetesArguments>,
    pub skip_from: bool,
}
