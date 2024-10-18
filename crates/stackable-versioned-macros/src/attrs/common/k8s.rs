use darling::{util::Flag, FromMeta};

/// This struct contains supported Kubernetes arguments.
///
/// Supported arguments are:
///
/// - `skip`, which controls skipping parts of the generation.
/// - `kind`, which allows overwriting the kind field of the CRD. This defaults to the struct name
///    (without the 'Spec' suffix).
/// - `group`, which sets the CRD group, usually the domain of the company.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct KubernetesArguments {
    pub(crate) skip: Option<KubernetesSkipArguments>,
    pub(crate) singular: Option<String>,
    pub(crate) plural: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) namespaced: Flag,
    pub(crate) group: String,
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
