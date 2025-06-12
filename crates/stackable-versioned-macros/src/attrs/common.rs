use std::ops::Deref;

use darling::{
    Error, FromMeta, Result,
    util::{Flag, Override as FlagOrOverride, SpannedValue},
};
use itertools::Itertools;
use k8s_version::Version;

pub trait CommonOptions {
    fn allow_unsorted(&self) -> Flag;
}

#[derive(Debug, FromMeta)]
#[darling(and_then = CommonRootArguments::validate)]
pub struct CommonRootArguments<T>
where
    T: CommonOptions + Default,
{
    #[darling(default)]
    pub options: T,

    #[darling(multiple, rename = "version")]
    pub versions: SpannedValue<Vec<VersionArguments>>,
}

impl<T> CommonRootArguments<T>
where
    T: CommonOptions + Default,
{
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

        if !self.options.allow_unsorted().is_present() && !is_sorted {
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
    pub skip: Option<SkipArguments>,
    pub doc: Option<String>,
    pub name: Version,
}

/// This struct contains supported common skip arguments.
///
/// Supported arguments are:
///
/// - `from` flag, which skips generating [`From`] implementations when provided.
#[derive(Clone, Debug, Default, FromMeta)]
pub struct SkipArguments {
    /// Whether the [`From`] implementation generation should be skipped for all versions of this
    /// container.
    pub from: Flag,
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
