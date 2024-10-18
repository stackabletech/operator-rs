//! This module contains attributes which can be used on containers (structs and enums).
//!
//! Generally there are two different containers, called "standalone" and "nested" based on which
//! context they are used in. Standalone containers define versioning directly on the struct or
//! enum. This is useful for single versioned containers. This type of versioning is fine as long
//! as there is no other versioned container in the same file using the same versions. If that is
//! the case, the generated modules will collide. There are two possible solutions for this: move
//! each versioned container into its own file or use the nested declarations. It should be noted
//! that there might be cases where it is fine to separate each container into its own file. One
//! such case is when each container serves distinctively different use-cases and provide numerous
//! associated items like functions.
//!
//! In cases where separate files are not desired, the nested mode can be used. The nested mode
//! allows to declare versions on a module which contains containers which shall be versioned
//! according to the defined versions. This approach allows defining multiple versioned containers
//! in the same file without module collisions.
//!
//! The attributes used must be tailored to both of these two modes, because not all arguments are
//! valid in all modes. As such different attributes allow different validation mechanisms. One
//! such an example is that nested containers must not define versions as the definition is done
//! on the module. This is in direct contrast to containers used in standalone mode.

use std::{cmp::Ordering, ops::Deref};

use darling::{
    util::{Flag, SpannedValue},
    Error, FromAttributes, FromMeta, Result,
};
use itertools::Itertools;

use crate::attrs::common::{KubernetesArguments, SkipArguments, VersionArguments};

/// This struct contains supported container attributes which can be applied to structs and enums.
///
/// Currently supported attributes are:
///
/// - `version`, which can occur one or more times. See [`VersionAttributes`].
/// - `k8s`, which enables Kubernetes specific features and allows customization if these features.
/// - `options`, which allow further customization of the generated code.
///    See [`StandaloneOptionArguments`].
#[derive(Debug, FromMeta)]
#[darling(and_then = StandaloneContainerAttributes::validate)]
pub(crate) struct StandaloneContainerAttributes {
    #[darling(multiple, rename = "version")]
    pub(crate) versions: SpannedValue<Vec<VersionArguments>>,

    #[darling(rename = "k8s")]
    pub(crate) kubernetes_args: Option<KubernetesArguments>,

    #[darling(default, rename = "options")]
    pub(crate) common_option_args: StandaloneOptionArguments,
}

impl StandaloneContainerAttributes {
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
        if !self.common_option_args.allow_unsorted.is_present() {
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
        let duplicate_versions = self
            .versions
            .iter()
            .duplicates_by(|e| e.name)
            .map(|e| e.name)
            .join(", ");

        if !duplicate_versions.is_empty() {
            return Err(Error::custom(format!(
                "attribute macro `#[versioned()]` contains duplicate versions: {duplicate_versions}",
            ))
            .with_span(&self.versions.span()));
        }

        // Ensure that the 'k8s' feature is enabled when the 'k8s()'
        // attribute is used.
        if self.kubernetes_args.is_some() && cfg!(not(feature = "k8s")) {
            return Err(Error::custom(
                "the `#[versioned(k8s())]` attribute can only be used when the `k8s` feature is enabled",
            ));
        }

        Ok(self)
    }
}

/// This struct contains supported option arguments for containers used in standalone mode.
///
/// Supported arguments are:
///
/// - `allow_unsorted`, which allows declaring versions in unsorted order, instead of enforcing
///    ascending order.
/// - `skip` option to skip generating various pieces of code.
#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct StandaloneOptionArguments {
    pub(crate) allow_unsorted: Flag,
    pub(crate) skip: Option<SkipArguments>,
}

#[derive(Debug, FromAttributes)]
#[darling(attributes(versioned))]
pub(crate) struct NestedContainerAttributes {
    #[darling(rename = "k8s")]
    pub(crate) kubernetes_args: Option<KubernetesArguments>,

    #[darling(default, rename = "options")]
    pub(crate) common_option_args: NestedOptionArguments,
}

#[derive(Clone, Debug, Default, FromMeta)]
pub(crate) struct NestedOptionArguments {
    pub(crate) skip: Option<SkipArguments>,
}
