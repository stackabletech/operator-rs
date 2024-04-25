use std::cmp::Ordering;

use darling::{util::SpannedValue, Error, FromField, FromMeta};
use k8s_version::Version;
use syn::{spanned::Spanned, Field, Ident};

use crate::{attrs::container::ContainerAttributes, consts::DEPRECATED_PREFIX};

/// This struct describes all available field attributes, as well as the field
/// name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`FieldAttributes::check_versions`]
/// function.
///
/// ### Field Rules
///
/// - A field can only ever be added once at most. A field not marked as 'added'
///   is part of the struct in every version until renamed or deprecated.
/// - A field can be renamed many times. That's why renames are stored in a
///   [`Vec`].
/// - A field can only be deprecated once. A field not marked as 'deprecated'
///   will be included up until the latest version.
#[derive(Debug, FromField)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = FieldAttributes::validate
)]
pub(crate) struct FieldAttributes {
    pub(crate) ident: Option<Ident>,
    pub(crate) added: Option<AddedAttributes>,

    #[darling(multiple, rename = "renamed")]
    pub(crate) renames: Vec<RenamedAttributes>,

    pub(crate) deprecated: Option<DeprecatedAttributes>,
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    pub(crate) since: SpannedValue<Version>,
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) from: SpannedValue<String>,
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) _note: SpannedValue<String>,
}

impl FieldAttributes {
    fn validate(mut self) -> Result<Self, Error> {
        match (&self.added, &self.renames, &self.deprecated) {
            // The derive macro prohibits the use of the 'added' and 'deprecated'
            // field action using the same version. This is because it doesn't
            // make sense to add a field and immediatly mark that field as
            // deprecated in the same version. Instead, fields should be
            // deprecated at least one version later.
            (Some(added), _, Some(deprecated)) => {
                if *added.since == *deprecated.since {
                    return Err(Error::custom(
                        "field cannot be marked as `added` and `deprecated` in the same version",
                    )
                    .with_span(&self.ident.span()));
                }
            }
            (Some(added), renamed, _) => {
                if renamed.iter().any(|r| *r.since == *added.since) {
                    return Err(Error::custom(
                        "field cannot be marked as `added` and `renamed` in the same version",
                    )
                    .with_span(&self.ident.span()));
                }
            }
            (_, renamed, Some(deprecated)) => {
                if renamed.iter().any(|r| *r.since == *deprecated.since) {
                    return Err(Error::custom(
                        "field cannot be marked as `deprecated` and `renamed` in the same version",
                    )
                    .with_span(&self.ident.span()));
                }
            }
            _ => {}
        }

        // Validate that renamed action versions are sorted to ensure consistent
        // code.
        let original = self.renames.clone();
        self.renames
            .sort_by(|lhs, rhs| lhs.since.partial_cmp(&rhs.since).unwrap_or(Ordering::Equal));

        for (index, version) in original.iter().enumerate() {
            if *version.since == *self.renames.get(index).unwrap().since {
                continue;
            }

            return Err(Error::custom(format!(
                "version of renames must be defined in ascending order (version `{}` is misplaced)",
                *version.since
            ))
            .with_span(&self.ident.span()));
        }

        // TODO (@Techassi): Add validation for renames so that renamed fields
        // match up and form a continous chain (eg. foo -> bar -> baz).

        // TODO (@Techassi): Add hint if a field is added in the first version
        // that it might be clever to remove the 'added' attribute.

        // Validate that actions use chronologically valid versions. If the
        // field was added (not included from the start), the version must be
        // less than version from the renamed and deprecated actions.
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        if let Some(added_version) = added_version {
            if !self.renames.iter().all(|r| *r.since > added_version) {
                return Err(Error::custom(format!(
                    "field was marked as `added` in version `{}` and thus all renames must use a higher version",
                    added_version
                ))
                .with_span(&self.ident.span()));
            }

            if let Some(deprecated_version) = deprecated_version {
                if added_version > deprecated_version {
                    return Err(Error::custom(format!(
                        "field was marked as `added` in version `{}` while being marked as `deprecated` in an earlier version `{}`",
                        added_version,
                        deprecated_version
                    )).with_span(&self.ident.span()));
                }
            }
        }

        // The same rule applies to renamed fields. Versions of renames must be
        // less than the deprecation version (if any).
        if let Some(deprecated_version) = deprecated_version {
            if !self.renames.iter().all(|r| *r.since < deprecated_version) {
                return Err(Error::custom(format!(
                    "field was marked as `deprecated` in version `{}` and thus all renames must use a lower version",
                    deprecated_version
                )).with_span(&self.ident.span()));
            }

            // Also check if the field starts with the prefix 'deprecated_'.
            if !self
                .ident
                .as_ref()
                .unwrap()
                .to_string()
                .starts_with(DEPRECATED_PREFIX)
            {
                return Err(Error::custom(
                    "field was marked as `deprecated` and thus must include the `deprecated_` prefix in its name",
                )
                .with_span(&self.ident.span()));
            }
        }

        Ok(self)
    }

    pub(crate) fn check_versions(
        &self,
        container_attrs: &ContainerAttributes,
        field: &Field,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?

        if let Some(added) = &self.added {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *added.since)
            {
                return Err(
                    Error::custom("field action `added` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident.span()));
            }
        }

        for rename in &self.renames {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *rename.since)
            {
                return Err(Error::custom("field action `renamed` uses version which was not declared via #[versioned(version)]"));
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *deprecated.since)
            {
                return Err(Error::custom("field action `deprecated` uses version which was not declared via #[versioned(version)]"));
            }
        }

        Ok(())
    }
}
