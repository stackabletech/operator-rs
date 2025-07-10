use std::ops::Deref;

use darling::{
    Error, FromMeta, Result,
    util::{Flag, Override as FlagOrOverride, SpannedValue},
};
use itertools::Itertools as _;
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Path, parse_quote};

#[derive(Debug, FromMeta)]
#[darling(and_then = ModuleAttributes::validate)]
pub struct ModuleAttributes {
    #[darling(multiple, rename = "version")]
    pub versions: SpannedValue<Vec<VersionArguments>>,

    #[darling(default)]
    pub crates: CrateArguments,

    #[darling(default)]
    pub options: ModuleOptions,

    #[darling(default)]
    pub skip: ModuleSkipArguments,
}

impl ModuleAttributes {
    fn validate(mut self) -> Result<Self> {
        let mut errors = Error::accumulator();

        if self.versions.is_empty() {
            errors.push(
                Error::custom("at least one or more `version`s must be defined")
                    .with_span(&self.versions.span()),
            );
        }

        let is_sorted = self.versions.iter().is_sorted_by_key(|v| v.name);

        // It needs to be sorted, even though the definition could be unsorted
        // (if allow_unsorted is set).
        self.versions.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        if !self.options.common.allow_unsorted.is_present() && !is_sorted {
            let versions = self.versions.iter().map(|v| v.name).join(", ");

            errors.push(Error::custom(format!(
                "versions must be defined in ascending order: {versions}",
            )));
        }

        let duplicate_versions: Vec<_> = self
            .versions
            .iter()
            .duplicates_by(|v| v.name)
            .map(|v| v.name)
            .collect();

        if !duplicate_versions.is_empty() {
            let versions = duplicate_versions.iter().join(", ");

            errors.push(Error::custom(format!(
                "contains duplicate versions: {versions}",
            )));
        }

        errors.finish_with(self)
    }
}

#[derive(Debug, Default, FromMeta)]
pub struct ModuleOptions {
    #[darling(flatten)]
    pub common: ModuleCommonOptions,

    #[darling(default, rename = "k8s")]
    pub kubernetes: KubernetesConfigOptions,
}

#[derive(Debug, Default, FromMeta)]
pub struct ModuleCommonOptions {
    pub allow_unsorted: Flag,
    pub preserve_module: Flag,
}

#[derive(Debug, Default, FromMeta)]
pub struct ModuleSkipArguments {
    pub from: Flag,
    pub object_from: Flag,
    pub merged_crd: Flag,
    pub try_convert: Flag,
}

/// This struct contains crate overrides to be passed to `#[kube]`.
#[derive(Clone, Debug, FromMeta)]
pub struct CrateArguments {
    #[darling(default = default_kube_core)]
    pub kube_core: Override<Path>,

    #[darling(default = default_kube_client)]
    pub kube_client: Override<Path>,

    #[darling(default = default_k8s_openapi)]
    pub k8s_openapi: Override<Path>,

    #[darling(default = default_schemars)]
    pub schemars: Override<Path>,

    #[darling(default = default_serde)]
    pub serde: Override<Path>,

    #[darling(default = default_serde_json)]
    pub serde_json: Override<Path>,

    #[darling(default = default_serde_yaml)]
    pub serde_yaml: Override<Path>,

    #[darling(default = default_versioned)]
    pub versioned: Override<Path>,
}

impl Default for CrateArguments {
    fn default() -> Self {
        Self {
            kube_core: default_kube_core(),
            kube_client: default_kube_client(),
            k8s_openapi: default_k8s_openapi(),
            schemars: default_schemars(),
            serde: default_serde(),
            serde_json: default_serde_json(),
            serde_yaml: default_serde_yaml(),
            versioned: default_versioned(),
        }
    }
}

fn default_kube_core() -> Override<Path> {
    Override::Default(parse_quote! { ::kube::core })
}

fn default_kube_client() -> Override<Path> {
    Override::Default(parse_quote! { ::kube::client })
}

fn default_k8s_openapi() -> Override<Path> {
    Override::Default(parse_quote! { ::k8s_openapi })
}

fn default_schemars() -> Override<Path> {
    Override::Default(parse_quote! { ::schemars })
}

fn default_serde() -> Override<Path> {
    Override::Default(parse_quote! { ::serde })
}

fn default_serde_json() -> Override<Path> {
    Override::Default(parse_quote! { ::serde_json })
}

fn default_serde_yaml() -> Override<Path> {
    Override::Default(parse_quote! { ::serde_yaml })
}

fn default_versioned() -> Override<Path> {
    Override::Default(parse_quote! { ::stackable_versioned })
}

impl ToTokens for CrateArguments {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut crate_overrides = TokenStream::new();

        let CrateArguments {
            kube_client: _,
            k8s_openapi,
            serde_json,
            kube_core,
            schemars,
            serde,
            ..
        } = self;

        if let Override::Explicit(k8s_openapi) = k8s_openapi {
            crate_overrides.extend(quote! { k8s_openapi = #k8s_openapi, });
        }

        if let Override::Explicit(serde_json) = serde_json {
            crate_overrides.extend(quote! { serde_json = #serde_json, });
        }

        if let Override::Explicit(kube_core) = kube_core {
            crate_overrides.extend(quote! { kube_core = #kube_core, });
        }

        if let Override::Explicit(schemars) = schemars {
            crate_overrides.extend(quote! { schemars = #schemars, });
        }

        if let Override::Explicit(serde) = serde {
            crate_overrides.extend(quote! { serde = #serde, });
        }

        if !crate_overrides.is_empty() {
            tokens.extend(quote! { , crates(#crate_overrides) });
        }
    }
}

#[derive(Clone, Default, Debug, FromMeta)]
pub struct KubernetesConfigOptions {
    pub experimental_conversion_tracking: Flag,
    pub enable_tracing: Flag,
}

/// This struct contains supported version arguments.
///
/// Supported arguments are:
///
/// - `name` of the version, like `v1alpha1`.
/// - `deprecated` flag to mark that version as deprecated.
/// - `skip` option to skip generating various pieces of code.
/// - `doc` option to add version-specific documentation.
#[derive(Clone, Debug, FromMeta)]
pub struct VersionArguments {
    pub deprecated: Option<FlagOrOverride<String>>,
    pub skip: Option<VersionSkipArguments>,
    pub doc: Option<String>,
    pub name: Version,
}

#[derive(Clone, Debug, FromMeta)]
pub struct VersionSkipArguments {
    pub from: Flag,
    pub object_from: Flag,
}

/// Wraps a value to indicate whether it is original or has been overridden.
#[derive(Clone, Debug)]
pub enum Override<T> {
    Default(T),
    Explicit(T),
}

impl<T> FromMeta for Override<T>
where
    T: FromMeta,
{
    fn from_meta(item: &syn::Meta) -> Result<Self> {
        FromMeta::from_meta(item).map(Override::Explicit)
    }
}

impl<T> Deref for Override<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self {
            Override::Default(inner) => inner,
            Override::Explicit(inner) => inner,
        }
    }
}
