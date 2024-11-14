use darling::{
    util::{Flag, Override, SpannedValue},
    Error, FromMeta, Result,
};
use itertools::Itertools;
use k8s_version::Version;

#[derive(Debug, FromMeta)]
#[darling(and_then = CommonRootArguments::validate)]
pub(crate) struct CommonRootArguments {
    #[darling(default)]
    pub(crate) options: RootOptions,

    #[darling(multiple, rename = "version")]
    pub(crate) versions: SpannedValue<Vec<VersionArguments>>,
}

impl CommonRootArguments {
    fn validate(mut self) -> Result<Self> {
        let mut errors = Error::accumulator();

        if self.versions.is_empty() {
            errors.push(
                Error::custom("at least one or more `version`s must be defined")
                    .with_span(&self.versions.span()),
            );
        }

        let is_sorted = self.versions.iter().is_sorted_by_key(|v| v.name);

        // It needs to be sorted, even tho the definition could be unsorted (if allow_unsorted is
        // set).
        self.versions.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        if !self.options.allow_unsorted.is_present() && !is_sorted {
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

#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct RootOptions {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip: Option<SkipArguments>,
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
pub(crate) struct VersionArguments {
    pub(crate) deprecated: Option<Override<String>>,
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
