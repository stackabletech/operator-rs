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

    pub fn generate_from_impl(
        &self,
        direction: Direction,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => {
                // TODO (@Techassi): Decide here (based on K8s args) what we want to generate
                s.generate_from_impl(direction, version, next_version, add_attributes)
            }
            Container::Enum(e) => {
                e.generate_from_impl(direction, version, next_version, add_attributes)
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

/// A collection of container idents used for different purposes.
#[derive(Debug)]
pub struct ContainerIdents {
    /// The original ident, or name, of the versioned container.
    pub original: IdentString,

    /// The ident used as a parameter.
    pub parameter: IdentString,
}

#[derive(Debug)]
pub struct KubernetesIdents {
    /// This ident removes the 'Spec' suffix present in the definition container.
    /// This ident is only used in the context of Kubernetes specific code.
    pub kind: IdentString,

    /// This ident uses the base Kubernetes ident to construct an appropriate ident
    /// for auto-generated status structs. This ident is only used in the context of
    /// Kubernetes specific code.
    pub status: IdentString,

    /// This ident uses the base Kubernetes ident to construct an appropriate ident
    /// for auto-generated version enums. This enum is used to select the stored
    /// api version when merging CRDs. This ident is only used in the context of
    /// Kubernetes specific code.
    pub version: IdentString,

    // TODO (@Techassi): Add comment
    pub parameter: IdentString,
}

impl From<Ident> for ContainerIdents {
    fn from(ident: Ident) -> Self {
        Self {
            parameter: ident.as_parameter_ident(),
            original: ident.into(),
        }
    }
}

impl KubernetesIdents {
    pub fn from(ident: &IdentString, arguments: &StructCrdArguments) -> Self {
        let kind = match &arguments.kind {
            Some(kind) => IdentString::from(Ident::new(kind, Span::call_site())),
            None => ident.as_cleaned_kubernetes_ident(),
        };

        let status = IdentString::from(format_ident!("{kind}StatusWithChangedValues"));
        let version = IdentString::from(format_ident!("{kind}Version"));
        let parameter = kind.as_parameter_ident();

        Self {
            parameter,
            version,
            status,
            kind,
        }
    }
}

#[derive(Debug)]
pub struct ContainerOptions {
    pub kubernetes_arguments: Option<KubernetesArguments>,
    pub skip_from: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Upgrade,
    Downgrade,
}
