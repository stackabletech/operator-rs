use std::collections::HashMap;

use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::{Span, TokenStream, TokenTree};
use quote::format_ident;
use syn::{Attribute, Ident};

use crate::{
    attrs::container::StructCrdArguments,
    codegen::{
        VersionDefinition,
        container::{r#enum::Enum, r#struct::Struct},
        module::ModuleGenerationContext,
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

#[derive(Debug, Default)]
pub struct ContainerTokens<'a> {
    pub versioned: HashMap<&'a Version, VersionedContainerTokens>,
    pub outer: TokenStream,
}

#[derive(Debug, Default)]
/// A collection of generated tokens for a container per version.
pub struct VersionedContainerTokens {
    /// The inner tokens are placed inside the version module. These tokens mostly only include the
    /// container definition with attributes, doc comments, etc.
    pub inner: TokenStream,

    /// These tokens are placed between version modules. These could technically be grouped together
    /// with the outer tokens, but it makes sense to keep them separate to achieve a more structured
    /// code generation. These tokens mostly only include `From` impls to convert between two versions
    pub between: TokenStream,
}

pub trait ExtendContainerTokens<'a, T> {
    fn extend_inner<I: IntoIterator<Item = T>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self;
    fn extend_between<I: IntoIterator<Item = T>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self;
    fn extend_outer<I: IntoIterator<Item = T>>(&mut self, streams: I) -> &mut Self;
}

impl<'a> ExtendContainerTokens<'a, TokenStream> for ContainerTokens<'a> {
    fn extend_inner<I: IntoIterator<Item = TokenStream>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self {
        self.versioned
            .entry(version)
            .or_default()
            .inner
            .extend(streams);
        self
    }

    fn extend_between<I: IntoIterator<Item = TokenStream>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self {
        self.versioned
            .entry(version)
            .or_default()
            .between
            .extend(streams);
        self
    }

    fn extend_outer<I: IntoIterator<Item = TokenStream>>(&mut self, streams: I) -> &mut Self {
        self.outer.extend(streams);
        self
    }
}

impl<'a> ExtendContainerTokens<'a, TokenTree> for ContainerTokens<'a> {
    fn extend_inner<I: IntoIterator<Item = TokenTree>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self {
        self.versioned
            .entry(version)
            .or_default()
            .inner
            .extend(streams);
        self
    }

    fn extend_between<I: IntoIterator<Item = TokenTree>>(
        &mut self,
        version: &'a Version,
        streams: I,
    ) -> &mut Self {
        self.versioned
            .entry(version)
            .or_default()
            .between
            .extend(streams);
        self
    }

    fn extend_outer<I: IntoIterator<Item = TokenTree>>(&mut self, streams: I) -> &mut Self {
        self.outer.extend(streams);
        self
    }
}

impl Container {
    // TODO (@Techassi): Only have a single function here. It should return and store all generated
    // tokens. It should also have access to a single GenerationContext, which provides all external
    // parameters which influence code generation.
    pub fn generate_tokens<'a>(
        &'a self,
        versions: &'a [VersionDefinition],
        ctx: ModuleGenerationContext<'a>,
    ) -> ContainerTokens<'a> {
        match self {
            Container::Struct(s) => s.generate_tokens(versions, ctx),
            Container::Enum(e) => e.generate_tokens(versions, ctx),
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
    pub skip_from: bool,
    pub skip_object_from: bool,
    pub skip_merged_crd: bool,
    pub skip_try_convert: bool,
}
