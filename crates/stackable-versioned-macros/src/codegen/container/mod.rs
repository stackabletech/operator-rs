use std::ops::Deref;

use darling::{util::IdentString, Result};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_quote, Attribute, Ident, ItemEnum, ItemStruct, Path, Visibility};

use crate::{
    attrs::{
        container::StandaloneContainerAttributes,
        k8s::{KubernetesArguments, KubernetesCrateArguments},
    },
    codegen::{
        container::{r#enum::Enum, r#struct::Struct},
        VersionDefinition,
    },
    utils::ContainerIdentExt,
};

mod r#enum;
mod r#struct;

/// Contains common container data shared between structs and enums.
pub(crate) struct CommonContainerData {
    /// Original attributes placed on the container, like `#[derive()]` or `#[cfg()]`.
    pub(crate) original_attributes: Vec<Attribute>,

    /// Different options which influence code generation.
    pub(crate) options: ContainerOptions,

    /// A collection of container idents used for different purposes.
    pub(crate) idents: ContainerIdents,
}

/// Supported types of containers, structs and enums.
///
/// Abstracting away with kind of container is generated makes it possible to create a list of
/// containers when the macro is used on modules. This enum provides functions to generate code
/// which then internally call the appropriate function based on the variant.
pub(crate) enum Container {
    Struct(Struct),
    Enum(Enum),
}

impl Container {
    /// Generates the container definition for the specified `version`.
    pub(crate) fn generate_definition(&self, version: &VersionDefinition) -> TokenStream {
        match self {
            Container::Struct(s) => s.generate_definition(version),
            Container::Enum(e) => e.generate_definition(version),
        }
    }

    /// Generates the container `From<Version> for NextVersion` implementation.
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
    pub(crate) fn generate_kubernetes_item(
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
    pub(crate) fn generate_kubernetes_merge_crds(
        &self,
        enum_variant_idents: &[IdentString],
        enum_variant_strings: &[String],
        fn_calls: &[TokenStream],
        is_nested: bool,
    ) -> Option<TokenStream> {
        match self {
            Container::Struct(s) => s.generate_kubernetes_merge_crds(
                enum_variant_idents,
                enum_variant_strings,
                fn_calls,
                is_nested,
            ),
            Container::Enum(_) => None,
        }
    }

    pub(crate) fn get_original_ident(&self) -> &Ident {
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
pub(crate) struct StandaloneContainer {
    versions: Vec<VersionDefinition>,
    container: Container,
    vis: Visibility,
}

impl StandaloneContainer {
    /// Creates a new versioned standalone struct.
    pub(crate) fn new_struct(
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

    /// Generate tokens containing every piece of code required for a standalone container.
    pub(crate) fn generate_tokens(&self) -> TokenStream {
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

                #from_impl
            });
        }

        tokens.extend(self.container.generate_kubernetes_merge_crds(
            &kubernetes_enum_variant_idents,
            &kubernetes_enum_variant_strings,
            &kubernetes_merge_crds_fn_calls,
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
    pub(crate) group: String,
    pub(crate) kind: Option<String>,
    pub(crate) singular: Option<String>,
    pub(crate) plural: Option<String>,
    pub(crate) namespaced: bool,
    // root
    pub(crate) crates: KubernetesCrateOptions,
    pub(crate) status: Option<String>,
    // derive
    // schema
    // scale
    // printcolumn
    pub(crate) shortname: Option<String>,
    // category
    // selectable
    // doc
    // annotation
    // label
    pub(crate) skip_merged_crd: bool,
}

impl From<KubernetesArguments> for KubernetesOptions {
    fn from(args: KubernetesArguments) -> Self {
        KubernetesOptions {
            group: args.group,
            kind: args.kind,
            singular: args.singular,
            plural: args.plural,
            namespaced: args.namespaced.is_present(),
            crates: args
                .crates
                .map_or_else(KubernetesCrateOptions::default, |crates| crates.into()),
            status: args.status,
            shortname: args.shortname,
            skip_merged_crd: args.skip.map_or(false, |s| s.merged_crd.is_present()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct KubernetesCrateOptions {
    pub(crate) kube_core: Override<Path>,
    pub(crate) k8s_openapi: Override<Path>,
    pub(crate) schemars: Override<Path>,
    pub(crate) serde: Override<Path>,
    pub(crate) serde_json: Override<Path>,
}

impl Default for KubernetesCrateOptions {
    fn default() -> Self {
        Self {
            k8s_openapi: Override::new_default(parse_quote! { ::k8s_openapi }),
            serde_json: Override::new_default(parse_quote! { ::serde_json }),
            kube_core: Override::new_default(parse_quote! { ::kube::core }),
            schemars: Override::new_default(parse_quote! { ::schemars }),
            serde: Override::new_default(parse_quote! { ::serde }),
        }
    }
}

impl From<KubernetesCrateArguments> for KubernetesCrateOptions {
    fn from(args: KubernetesCrateArguments) -> Self {
        let mut crate_options = Self::default();

        if let Some(k8s_openapi) = args.k8s_openapi {
            crate_options.k8s_openapi = Override::new_custom(k8s_openapi);
        }

        if let Some(serde_json) = args.serde_json {
            crate_options.serde_json = Override::new_custom(serde_json);
        }

        if let Some(kube_core) = args.kube_core {
            crate_options.kube_core = Override::new_custom(kube_core);
        }

        if let Some(schemars) = args.schemars {
            crate_options.schemars = Override::new_custom(schemars);
        }

        if let Some(serde) = args.serde {
            crate_options.serde = Override::new_custom(serde);
        }

        crate_options
    }
}

impl ToTokens for KubernetesCrateOptions {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut crate_overrides = TokenStream::new();

        let KubernetesCrateOptions {
            k8s_openapi,
            serde_json,
            kube_core,
            schemars,
            serde,
        } = self;

        if let Some(k8s_openapi) = k8s_openapi.get_if_overridden() {
            crate_overrides.extend(quote! { k8s_openapi = #k8s_openapi, });
        }

        if let Some(serde_json) = serde_json.get_if_overridden() {
            crate_overrides.extend(quote! { serde_json = #serde_json, });
        }

        if let Some(kube_core) = kube_core.get_if_overridden() {
            crate_overrides.extend(quote! { kube_core = #kube_core, });
        }

        if let Some(schemars) = schemars.get_if_overridden() {
            crate_overrides.extend(quote! { schemars = #schemars, });
        }

        if let Some(serde) = serde.get_if_overridden() {
            crate_overrides.extend(quote! { serde = #serde, });
        }

        if !crate_overrides.is_empty() {
            tokens.extend(quote! { , crates(#crate_overrides) });
        }
    }
}

/// Wraps a value to indicate whether it is original or has been overridden.
#[derive(Debug)]
pub(crate) struct Override<T> {
    is_overridden: bool,
    inner: T,
}

impl<T> Override<T> {
    /// Mark a value as a default.
    ///
    /// This is used to indicate that the value is a default and was not overridden.
    pub(crate) fn new_default(inner: T) -> Self {
        Override {
            is_overridden: false,
            inner,
        }
    }

    /// Mark a value as overridden.
    ///
    /// This is used to indicate that the value was overridden and not the default.
    pub(crate) fn new_custom(inner: T) -> Self {
        Override {
            is_overridden: true,
            inner,
        }
    }

    pub(crate) fn get_if_overridden(&self) -> Option<&T> {
        match &self.is_overridden {
            true => Some(&self.inner),
            false => None,
        }
    }
}

impl<T> Deref for Override<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
