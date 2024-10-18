use darling::{util::Flag, FromMeta};
use k8s_version::Version;

mod container;
mod item;
mod k8s;
mod module;

pub(crate) use container::*;
pub(crate) use item::*;
pub(crate) use k8s::*;
pub(crate) use module::*;

/// This struct contains supported version arguments.
///
/// Supported arguments are:
///
/// - `name` of the version, like `v1alpha1`.
/// - `deprecated` flag to mark that version as deprecated.
/// - `skip` option to skip generating various pieces of code.
/// - `doc` option to add version-specific documentation.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct VersionArguments {
    pub(crate) deprecated: Flag,
    pub(crate) name: Version,
    pub(crate) skip: Option<SkipArguments>,
    pub(crate) doc: Option<String>,
}

/// This struct contains supported common skip arguments.
///
/// Supported arguments are:
///
/// - `from` flag, which skips generating [`From`] implementations when provided.
#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct SkipArguments {
    /// Whether the [`From`] implementation generation should be skipped for all versions of this
    /// container.
    pub(crate) from: Flag,
}
