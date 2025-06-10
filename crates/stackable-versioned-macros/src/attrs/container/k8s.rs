use darling::{FromMeta, util::Flag};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Path, parse_quote};

use crate::attrs::common::Override;

/// This struct contains supported Kubernetes arguments.
///
/// The arguments are passed through to the `#[kube]` attribute. More details can be found in the
/// official docs: <https://docs.rs/kube/latest/kube/derive.CustomResource.html>.
///
/// Supported arguments are:
///
/// - `group`: Set the group of the CR object, usually the domain of the company.
///   This argument is Required.
/// - `kind`: Override the kind field of the CR object. This defaults to the struct
///    name (without the 'Spec' suffix).
/// - `singular`: Set the singular name of the CR object.
/// - `plural`: Set the plural name of the CR object.
/// - `namespaced`: Indicate that this is a namespaced scoped resource rather than a
///    cluster scoped resource.
/// - `crates`: Override specific crates.
/// - `status`: Set the specified struct as the status subresource.
/// - `shortname`: Set a shortname for the CR object. This can be specified multiple
///   times.
/// - `skip`: Controls skipping parts of the generation.
#[derive(Clone, Debug, FromMeta)]
pub struct KubernetesArguments {
    pub group: String,
    pub kind: Option<String>,
    pub singular: Option<String>,
    pub plural: Option<String>,
    pub namespaced: Flag,
    // root
    #[darling(default)]
    pub crates: KubernetesCrateArguments,
    pub status: Option<Path>,
    // derive
    // schema
    // scale
    // printcolumn
    #[darling(multiple, rename = "shortname")]
    pub shortnames: Vec<String>,
    // category
    // selectable
    // doc
    // annotation
    // label
    pub skip: Option<KubernetesSkipArguments>,

    #[darling(default)]
    pub options: KubernetesConfigOptions,
}

/// This struct contains supported kubernetes skip arguments.
///
/// Supported arguments are:
///
/// - `merged_crd` flag, which skips generating the `crd()` and `merged_crd()` functions are
///    generated.
#[derive(Clone, Debug, FromMeta)]
pub struct KubernetesSkipArguments {
    /// Whether the `crd()` and `merged_crd()` generation should be skipped for
    /// this container.
    pub merged_crd: Flag,
}

/// This struct contains crate overrides to be passed to `#[kube]`.
#[derive(Clone, Debug, FromMeta)]
pub struct KubernetesCrateArguments {
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

    #[darling(default = default_versioned)]
    pub versioned: Override<Path>,
}

impl Default for KubernetesCrateArguments {
    fn default() -> Self {
        Self {
            kube_core: default_kube_core(),
            kube_client: default_kube_client(),
            k8s_openapi: default_k8s_openapi(),
            schemars: default_schemars(),
            serde: default_serde(),
            serde_json: default_serde_json(),
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

fn default_versioned() -> Override<Path> {
    Override::Default(parse_quote! { ::stackable_versioned })
}

impl ToTokens for KubernetesCrateArguments {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut crate_overrides = TokenStream::new();

        let KubernetesCrateArguments {
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
