use std::{cmp::Ordering, collections::HashSet, ops::Deref};

use darling::{
    util::{Flag, SpannedValue},
    Error, FromDeriveInput, FromMeta, Result,
};
use k8s_version::Version;

/// This struct contains supported container attributes.
///
/// Currently supported atttributes are:
///
/// - `version`, which can occur one or more times. See [`VersionAttributes`].
/// - `options`, which allow further customization of the generated code. See [`ContainerOptions`].
#[derive(Clone, Debug, FromDeriveInput)]
#[darling(
    attributes(versioned),
    supports(struct_named),
    forward_attrs(allow, doc, cfg, serde),
    and_then = ContainerAttributes::validate
)]
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
                "attribute `#[versioned()]` must contain at least one `version`",
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
                if version.name == self.versions.get(index).unwrap().name {
                    continue;
                }

                return Err(Error::custom(format!(
                    "versions in `#[versioned()]` must be defined in ascending order (version `{name}` is misplaced)",
                    name = version.name
                )));
            }
        }

        // Ensure every version is unique and isn't declared multiple times. This
        // is inspired by the itertools all_unique function.
        let mut unique = HashSet::new();

        for version in &*self.versions {
            if !unique.insert(version.name) {
                return Err(Error::custom(format!(
                    "attribute `#[versioned()]` contains duplicate version `name`: {name}",
                    name = version.name
                ))
                .with_span(&self.versions.span()));
            }
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
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct VersionAttributes {
    pub(crate) deprecated: Flag,
    pub(crate) name: Version,
}

/// This struct contains supported container options.
///
/// Supported options are:
///
/// - `allow_unsorted`, which allows declaring versions in unsorted order,
///   instead of enforcing ascending order.
#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct ContainerOptions {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip_from: Flag,
}
