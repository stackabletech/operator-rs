use darling::{FromMeta, util::Flag};
use syn::Path;

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
    pub crates: Option<KubernetesCrateArguments>,
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
    pub options: RawKubernetesOptions,
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
    pub kube_core: Option<Path>,
    pub kube_client: Option<Path>,
    pub k8s_openapi: Option<Path>,
    pub schemars: Option<Path>,
    pub serde: Option<Path>,
    pub serde_json: Option<Path>,
    pub versioned: Option<Path>,
}

#[derive(Clone, Default, Debug, FromMeta)]
pub struct RawKubernetesOptions {
    pub experimental_conversion_tracking: Flag,
}
