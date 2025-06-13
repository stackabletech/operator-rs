use darling::{Result, util::IdentString};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Ident, ItemEnum, ItemStruct, Visibility};

use crate::{
    attrs::container::{StandaloneContainerAttributes, k8s::KubernetesArguments},
    codegen::{
        KubernetesTokens, VersionDefinition,
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

    /// Generates Kubernetes specific code for the container.
    ///
    /// This includes CRD merging, CRD conversion, and the conversion tracking status struct.
    pub fn generate_kubernetes_code(
        &self,
        versions: &[VersionDefinition],
        tokens: &KubernetesTokens,
        vis: &Visibility,
        is_nested: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_code(versions, tokens, vis, is_nested),
            Container::Enum(_) => None,
        }
    }

    /// Generates KUbernetes specific code for individual versions.
    pub fn generate_kubernetes_version_items(
        &self,
        version: &VersionDefinition,
    ) -> Option<(TokenStream, IdentString, TokenStream, String)> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_version_items(version),
            Container::Enum(_) => None,
        }
    }

    /// Returns the original ident of the container.
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

        let mut kubernetes_tokens = KubernetesTokens::default();
        let mut tokens = TokenStream::new();

        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let container_definition = self.container.generate_definition(version);
            let module_ident = &version.idents.module;

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

            // Generate Kubernetes specific code (for a particular version) which is placed outside
            // of the container definition.
            if let Some(items) = self.container.generate_kubernetes_version_items(version) {
                kubernetes_tokens.push(items);
            }

            tokens.extend(quote! {
                #[automatically_derived]
                #deprecated_attribute
                #vis mod #module_ident {
                    use super::*;
                    #container_definition
                }

                #upgrade_from_impl
                #downgrade_from_impl
            });
        }

        // Finally add tokens outside of the container definitions
        tokens.extend(self.container.generate_kubernetes_code(
            &self.versions,
            &kubernetes_tokens,
            vis,
            false,
        ));

        tokens
    }
}

/// A collection of container idents used for different purposes.
#[derive(Debug)]
pub struct ContainerIdents {
    /// This ident removes the 'Spec' suffix present in the definition container.
    /// This ident is only used in the context of Kubernetes specific code.
    pub kubernetes: IdentString,

    /// This ident uses the base Kubernetes ident to construct an appropriate ident
    /// for auto-generated status structs. This ident is only used in the context of
    /// Kubernetes specific code.
    pub kubernetes_status: IdentString,

    /// This ident uses the base Kubernetes ident to construct an appropriate ident
    /// for auto-generated version enums. This enum is used to select the stored
    /// api version when merging CRDs. This ident is only used in the context of
    /// Kubernetes specific code.
    pub kubernetes_version: IdentString,

    // TODO (@Techassi): Add comment
    pub kubernetes_parameter: IdentString,

    /// The original ident, or name, of the versioned container.
    pub original: IdentString,

    /// The ident used as a parameter.
    pub parameter: IdentString,
}

impl ContainerIdents {
    pub fn from(ident: Ident, kubernetes_arguments: Option<&KubernetesArguments>) -> Self {
        let kubernetes = match kubernetes_arguments {
            Some(args) => match &args.kind {
                Some(kind) => IdentString::from(Ident::new(kind, Span::call_site())),
                None => ident.as_cleaned_kubernetes_ident(),
            },
            None => ident.as_cleaned_kubernetes_ident(),
        };

        let kubernetes_status =
            IdentString::from(format_ident!("{kubernetes}StatusWithChangedValues"));

        let kubernetes_version = IdentString::from(format_ident!("{kubernetes}Version"));
        let kubernetes_parameter = kubernetes.as_parameter_ident();

        Self {
            parameter: ident.as_parameter_ident(),
            original: ident.into(),
            kubernetes_parameter,
            kubernetes_version,
            kubernetes_status,
            kubernetes,
        }
    }
}

#[derive(Debug)]
pub struct ContainerOptions {
    pub kubernetes_arguments: Option<KubernetesArguments>,
    pub skip_from: bool,
}
