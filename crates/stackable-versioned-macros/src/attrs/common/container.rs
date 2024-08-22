use std::{cmp::Ordering, ops::Deref};

use darling::{
    util::{Flag, SpannedValue},
    Error, FromMeta, Result,
};
use itertools::Itertools;
use k8s_version::Version;

/// This struct contains supported container attributes.
///
/// Currently supported attributes are:
///
/// - `version`, which can occur one or more times. See [`VersionAttributes`].
/// - `options`, which allow further customization of the generated code. See [`ContainerOptions`].
#[derive(Debug, FromMeta)]
#[darling(and_then = ContainerAttributes::validate)]
pub(crate) struct ContainerAttributes {
    #[darling(multiple, rename = "version")]
    pub(crate) versions: SpannedValue<Vec<VersionAttributes>>,

    #[darling(default)]
    pub(crate) options: ContainerOptions,
}

impl ContainerAttributes {
    fn validate(mut self) -> Result<Self> {
        // Most of the validation for individual version strings is done by the
        // k8s-version crate. That's why the code below only checks that at
        // least one version is defined, they are defined in order (to ensure
        // code consistency) and that all declared versions are unique.

        // If there are no versions defined, the derive macro errors out. There
        // should be at least one version if the derive macro is used.
        if self.versions.is_empty() {
            return Err(Error::custom(
                "attribute macro `#[versioned()]` must contain at least one `version`",
            )
            .with_span(&self.versions.span()));
        }

        // NOTE (@Techassi): Do we even want to allow to opt-out of this?

        // Ensure that versions are defined in sorted (ascending) order to keep
        // code consistent.
        if !self.options.allow_unsorted.is_present() {
            let original = self.versions.deref().clone();
            self.versions
                .sort_by(|lhs, rhs| lhs.name.partial_cmp(&rhs.name).unwrap_or(Ordering::Equal));

            for (index, version) in original.iter().enumerate() {
                if version.name
                    == self
                        .versions
                        .get(index)
                        .expect("internal error: version at that index must exist")
                        .name
                {
                    continue;
                }

                return Err(Error::custom(format!(
                    "versions in `#[versioned()]` must be defined in ascending order (version `{name}` is misplaced)",
                    name = version.name
                )));
            }
        }

        // TODO (@Techassi): Add validation for skip(from) for last version,
        // which will skip nothing, because nothing is generated in the first
        // place.

        // Ensure every version is unique and isn't declared multiple times.
        let duplicates = self
            .versions
            .iter()
            .duplicates_by(|e| e.name)
            .map(|e| e.name)
            .join(", ");

        if !duplicates.is_empty() {
            return Err(Error::custom(format!(
                "attribute macro `#[versioned()]` contains duplicate versions: {duplicates}",
            ))
            .with_span(&self.versions.span()));
        }

        Ok(self)
    }
}

/// This struct contains supported version options.
///
/// Supported options are:
///
/// - `name` of the version, like `v1alpha1`.
/// - `deprecated` flag to mark that version as deprecated.
/// - `skip` option to skip generating various pieces of code.
/// - `doc` option to add version-specific documentation.
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct VersionAttributes {
    pub(crate) deprecated: Flag,
    pub(crate) name: Version,
    pub(crate) skip: Option<SkipOptions>,
    pub(crate) doc: Option<String>,
}

/// This struct contains supported container options.
///
/// Supported options are:
///
/// - `allow_unsorted`, which allows declaring versions in unsorted order,
///   instead of enforcing ascending order.
/// - `skip` option to skip generating various pieces of code.
#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct ContainerOptions {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip: Option<SkipOptions>,
}

/// This struct contains supported skip options.
///
/// Supported options are:
///
/// - `from` flag, which skips generating [`From`] implementations when provided.
#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct SkipOptions {
    pub(crate) from: Flag,
}
