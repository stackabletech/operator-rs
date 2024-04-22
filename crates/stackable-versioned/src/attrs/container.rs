use std::{cmp::Ordering, collections::HashSet, ops::Deref};

use darling::{
    util::{Flag, SpannedValue},
    Error, FromDeriveInput, FromMeta,
};
use k8s_version::Version;

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
    pub(crate) options: VersionOptions,
}

impl ContainerAttributes {
    fn validate(mut self) -> darling::Result<Self> {
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
                    "versions in `#[versioned()]` must be defined in ascending order (version `{}` is misplaced)",
                    version.name
                )));
            }
        }

        // Ensure every version is unique and isn't declared multiple times. This
        // is inspired by the itertools all_unique function.
        let mut unique = HashSet::new();
        if !self
            .versions
            .iter()
            .all(move |elem| unique.insert(elem.name))
        {
            return Err(Error::custom(
                "attribute `#[versioned()]` contains one or more `version`s with a duplicate `name`",
            )
            .with_span(&self.versions.span()));
        }

        Ok(self)
    }
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct VersionAttributes {
    pub(crate) deprecated: Flag,
    pub(crate) name: Version,
}

#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct VersionOptions {
    pub(crate) allow_unsorted: Flag,
}
