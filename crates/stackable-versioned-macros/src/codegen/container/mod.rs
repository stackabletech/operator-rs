use std::ops::Deref;

use darling::{Result, util::IdentString};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Attribute, Ident, ItemEnum, ItemStruct, Path, Visibility, parse_quote};

use crate::{
    attrs::{
        container::StandaloneContainerAttributes,
        k8s::{KubernetesArguments, KubernetesCrateArguments, RawKubernetesOptions},
    },
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
pub enum Container {
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
pub(crate) struct ContainerIdents {
    /// The ident used in the context of Kubernetes specific code. This ident
    /// removes the 'Spec' suffix present in the definition container.
    pub kubernetes: IdentString,

    /// The original ident, or name, of the versioned container.
    pub original: IdentString,

    /// The ident used in the [`From`] impl.
    pub from: IdentString,
}

impl ContainerIdents {
    pub(crate) fn from(ident: Ident, kubernetes_options: Option<&KubernetesOptions>) -> Self {
        let kubernetes = kubernetes_options.map_or_else(
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
    pub kubernetes_options: Option<KubernetesOptions>,
    pub skip_from: bool,
}

// TODO (@Techassi): Get rid of this whole mess. There should be an elegant way of using the
// attributes directly (with all defaults set and validation done).
#[derive(Debug)]
pub struct KubernetesOptions {
    pub group: String,
    pub kind: Option<String>,
    pub singular: Option<String>,
    pub plural: Option<String>,
    pub namespaced: bool,
    // root
    pub crates: KubernetesCrateOptions,
    pub status: Option<Path>,
    // derive
    // schema
    // scale
    // printcolumn
    pub shortnames: Vec<String>,
    // category
    // selectable
    // doc
    // annotation
    // label
    pub skip_merged_crd: bool,
    pub config_options: KubernetesConfigOptions,
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
            shortnames: args.shortnames,
            skip_merged_crd: args.skip.is_some_and(|s| s.merged_crd.is_present()),
            config_options: args.options.into(),
        }
    }
}

#[derive(Debug)]
pub struct KubernetesCrateOptions {
    pub kube_client: Override<Path>,
    pub kube_core: Override<Path>,
    pub k8s_openapi: Override<Path>,
    pub schemars: Override<Path>,
    pub serde: Override<Path>,
    pub serde_json: Override<Path>,
    pub versioned: Override<Path>,
}

impl Default for KubernetesCrateOptions {
    fn default() -> Self {
        Self {
            versioned: Override::Default(parse_quote! { ::stackable_versioned }),
            kube_client: Override::Default(parse_quote! { ::kube::client }),
            k8s_openapi: Override::Default(parse_quote! { ::k8s_openapi }),
            serde_json: Override::Default(parse_quote! { ::serde_json }),
            kube_core: Override::Default(parse_quote! { ::kube::core }),
            schemars: Override::Default(parse_quote! { ::schemars }),
            serde: Override::Default(parse_quote! { ::serde }),
        }
    }
}

impl From<KubernetesCrateArguments> for KubernetesCrateOptions {
    fn from(args: KubernetesCrateArguments) -> Self {
        let mut crate_options = Self::default();

        if let Some(k8s_openapi) = args.k8s_openapi {
            crate_options.k8s_openapi = Override::Overridden(k8s_openapi);
        }

        if let Some(serde_json) = args.serde_json {
            crate_options.serde_json = Override::Overridden(serde_json);
        }

        if let Some(kube_core) = args.kube_core {
            crate_options.kube_core = Override::Overridden(kube_core);
        }

        if let Some(kube_client) = args.kube_client {
            crate_options.kube_client = Override::Overridden(kube_client);
        }

        if let Some(schemars) = args.schemars {
            crate_options.schemars = Override::Overridden(schemars);
        }

        if let Some(serde) = args.serde {
            crate_options.serde = Override::Overridden(serde);
        }

        if let Some(versioned) = args.versioned {
            crate_options.versioned = Override::Overridden(versioned);
        }

        crate_options
    }
}

impl ToTokens for KubernetesCrateOptions {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut crate_overrides = TokenStream::new();

        let KubernetesCrateOptions {
            kube_client: _,
            k8s_openapi,
            serde_json,
            kube_core,
            schemars,
            serde,
            ..
        } = self;

        if let Override::Overridden(k8s_openapi) = k8s_openapi {
            crate_overrides.extend(quote! { k8s_openapi = #k8s_openapi, });
        }

        if let Override::Overridden(serde_json) = serde_json {
            crate_overrides.extend(quote! { serde_json = #serde_json, });
        }

        if let Override::Overridden(kube_core) = kube_core {
            crate_overrides.extend(quote! { kube_core = #kube_core, });
        }

        if let Override::Overridden(schemars) = schemars {
            crate_overrides.extend(quote! { schemars = #schemars, });
        }

        if let Override::Overridden(serde) = serde {
            crate_overrides.extend(quote! { serde = #serde, });
        }

        if !crate_overrides.is_empty() {
            tokens.extend(quote! { , crates(#crate_overrides) });
        }
    }
}

/// Wraps a value to indicate whether it is original or has been overridden.
#[derive(Debug)]
pub enum Override<T> {
    Default(T),
    Overridden(T),
}

impl<T> Deref for Override<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self {
            Override::Default(inner) => inner,
            Override::Overridden(inner) => inner,
        }
    }
}

#[derive(Debug)]
pub struct KubernetesConfigOptions {
    experimental_conversion_tracking: bool,
}

impl From<RawKubernetesOptions> for KubernetesConfigOptions {
    fn from(options: RawKubernetesOptions) -> Self {
        Self {
            experimental_conversion_tracking: options.experimental_conversion_tracking.is_present(),
        }
    }
}
