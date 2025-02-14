use darling::{util::Flag, FromMeta};
use syn::Path;

/// This struct contains supported Kubernetes arguments.
///
/// The arguments are passed through to the `#[kube]` attribute. More details can be found in the
/// official docs: <https://docs.rs/kube/latest/kube/derive.CustomResource.html>.
///
/// Supported arguments are:
///
/// - `group`, which sets the CRD group, usually the domain of the company.
/// - `kind`, which allows overwriting the kind field of the CRD. This defaults to the struct name
///    (without the 'Spec' suffix).
/// - `singular`, to specify the singular name of the CR object.
/// - `plural`, to specify the plural name of the CR object.
/// - `namespaced`, to specify that this is a namespaced resource rather than cluster level.
/// - `crates`: Override specific crates.
/// - `status`: Sets the specified struct as the status subresource.
/// - `shortname`: Sets a shortname for the CRD. This can be specified multiple times.
/// - `skip`, which controls skipping parts of the generation.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct KubernetesArguments {
    pub(crate) group: String,
    pub(crate) kind: Option<String>,
    pub(crate) singular: Option<String>,
    pub(crate) plural: Option<String>,
    pub(crate) namespaced: Flag,
    // root
    pub(crate) crates: Option<KubernetesCrateArguments>,
    pub(crate) status: Option<String>,
    // derive
    // schema
    // scale
    // printcolumn
    #[darling(multiple, rename = "shortname")]
    pub(crate) shortnames: Vec<String>,
    // category
    // selectable
    // doc
    // annotation
    // label
    pub(crate) skip: Option<KubernetesSkipArguments>,
}

/// This struct contains supported kubernetes skip arguments.
///
/// Supported arguments are:
///
/// - `merged_crd` flag, which skips generating the `crd()` and `merged_crd()` functions are
///    generated.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct KubernetesSkipArguments {
    /// Whether the `crd()` and `merged_crd()` generation should be skipped for
    /// this container.
    pub(crate) merged_crd: Flag,
}

/// This struct contains crate overrides to be passed to `#[kube]`.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct KubernetesCrateArguments {
    pub(crate) kube_core: Option<Path>,
    pub(crate) k8s_openapi: Option<Path>,
    pub(crate) schemars: Option<Path>,
    pub(crate) serde: Option<Path>,
    pub(crate) serde_json: Option<Path>,
}
