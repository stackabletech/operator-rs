use darling::{util::IdentString, Result};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Ident, ItemEnum, ItemStruct, Visibility};

use crate::{
    attrs::{container::StandaloneContainerAttributes, k8s::KubernetesArguments},
    codegen::{
        container::{r#enum::Enum, r#struct::Struct},
        VersionDefinition,
    },
    utils::ContainerIdentExt,
};

mod r#enum;
mod r#struct;

pub(crate) struct CommonContainerData {
    /// Original attributes placed on the container, like `#[derive()]` or `#[cfg()]`.
    pub(crate) original_attributes: Vec<Attribute>,

    /// Different options which influence code generation.
    pub(crate) options: ContainerOptions,

    /// A collection of container idents used for different purposes.
    pub(crate) idents: ContainerIdents,
}

pub(crate) enum Container {
    Struct(Struct),
    Enum(Enum),
}

impl Container {
    pub(crate) fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        match self {
            Container::Struct(s) => s.generate_definition(version),
            Container::Enum(e) => e.generate_definition(version),
        }
    }

    pub(crate) fn generate_from_impl(
        &self,
        version: &VersionDefinition,
        next_version: Option<&VersionDefinition>,
        add_attributes: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => s.generate_from_impl(version, next_version, add_attributes),
            Container::Enum(e) => e.generate_from_impl(version, next_version, add_attributes),
        }
    }

    pub(crate) fn generate_kubernetes_item(
        &self,
        version: &VersionDefinition,
    ) -> Option<(IdentString, TokenStream)> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_item(version),
            Container::Enum(_) => None,
        }
    }

    pub(crate) fn generate_kubernetes_merge_crds(
        &self,
        enum_variants: Vec<IdentString>,
        fn_calls: Vec<TokenStream>,
        is_nested: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => {
                s.generate_kubernetes_merge_crds(enum_variants, fn_calls, is_nested)
            }
            Container::Enum(_) => None,
        }
    }
}

pub(crate) struct StandaloneContainer {
    versions: Vec<VersionDefinition>,
    container: Container,
    vis: Visibility,
}

impl StandaloneContainer {
    pub(crate) fn new_struct(
        item_struct: ItemStruct,
        attributes: StandaloneContainerAttributes,
    ) -> Result<Self> {
        // TODO (@Techassi): Only pass the fields we need from item struct instead of moving as a whole
        let versions: Vec<_> = (&attributes).into();
        let vis = item_struct.vis.clone();

        let container = Container::new_standalone_struct(item_struct, attributes, &versions)?;

        Ok(Self {
            container,
            versions,
            vis,
        })
    }

    pub(crate) fn new_enum(
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

    pub(crate) fn generate_tokens(&self) -> TokenStream {
        let vis = &self.vis;

        let mut tokens = TokenStream::new();

        let mut kubernetes_merge_crds_fn_calls = Vec::new();
        let mut kubernetes_enum_variants = Vec::new();

        let mut versions = self.versions.iter().peekable();

        while let Some(version) = versions.next() {
            let container_definition = self.container.generate_definition(version);

            // NOTE (@Techassi): Using '.copied()' here does not copy or clone the data, but instead
            // removes one level of indirection of the double reference '&&'.
            let from_impl =
                self.container
                    .generate_from_impl(version, versions.peek().copied(), false);

            // Add the #[deprecated] attribute when the version is marked as deprecated.
            let deprecated_attribute = version
                .deprecated
                .as_ref()
                .map(|note| quote! { #[deprecated = #note] });

            // Generate Kubernetes specific code which is placed outside of the container
            // definition.
            if let Some((enum_variant, fn_call)) = self.container.generate_kubernetes_item(version)
            {
                kubernetes_merge_crds_fn_calls.push(fn_call);
                kubernetes_enum_variants.push(enum_variant);
            }

            let version_ident = &version.ident;

            tokens.extend(quote! {
                #[automatically_derived]
                #deprecated_attribute
                #vis mod #version_ident {
                    use super::*;
                    #container_definition
                }

                #from_impl
            });
        }

        tokens.extend(self.container.generate_kubernetes_merge_crds(
            kubernetes_enum_variants,
            kubernetes_merge_crds_fn_calls,
            false,
        ));

        tokens
    }
}

/// A collection of container idents used for different purposes.
#[derive(Debug)]
pub(crate) struct ContainerIdents {
    /// The ident used in the context of Kubernetes specific code. This ident
    /// removes the 'Spec' suffix present in the definition container.
    pub(crate) kubernetes: IdentString,

    /// The original ident, or name, of the versioned container.
    pub(crate) original: IdentString,

    /// The ident used in the [`From`] impl.
    pub(crate) from: IdentString,
}

impl From<Ident> for ContainerIdents {
    fn from(ident: Ident) -> Self {
        Self {
            kubernetes: ident.as_cleaned_kubernetes_ident(),
            from: ident.as_from_impl_ident(),
            original: ident.into(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ContainerOptions {
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

impl From<KubernetesArguments> for KubernetesOptions {
    fn from(args: KubernetesArguments) -> Self {
        KubernetesOptions {
            skip_merged_crd: args.skip.map_or(false, |s| s.merged_crd.is_present()),
            namespaced: args.namespaced.is_present(),
            singular: args.singular,
            plural: args.plural,
            group: args.group,
            kind: args.kind,
        }
    }
}
