use darling::{util::SpannedValue, Error, FromField, FromMeta};
use k8s_version::Version;
use syn::{spanned::Spanned, Field, Ident};

use crate::gen::version::ContainerVersion;

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
    ident: Option<Ident>,
    added: Option<AddedAttributes>,

    #[darling(multiple)]
    renamed: Vec<RenamedAttributes>,

    deprecated: Option<DeprecatedAttributes>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    pub(crate) since: SpannedValue<Version>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) _from: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) _note: SpannedValue<String>,
}

impl FieldAttributes {
    pub fn validate(self) -> Result<Self, Error> {
        match (&self.added, &self.renamed, &self.deprecated) {
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

        // Validate that actions use chronologically valid versions. If the
        // field was added (not included from the start), the version must be
        // less than version from the renamed and deprecated actions.
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        if let Some(added_version) = added_version {
            if !self.renamed.iter().all(|r| *r.since > added_version) {
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

        // TODO (@Techassi): Add validation for renames so that renamed fields
        // match up and form a continous chain (eg. foo -> bar -> baz).

        // The same rule applies to renamed fields. Versions of renames must be
        // less than the deprecation version (if any).
        if let Some(deprecated_version) = deprecated_version {
            if !self.renamed.iter().all(|r| *r.since < deprecated_version) {
                return Err(Error::custom(format!(
                    "field was marked as `deprecated` in version `{}` and thus all renames must use a lower version",
                    deprecated_version
                )));
            }
        }

        Ok(self)
    }

    pub(crate) fn check_versions(
        &self,
        versions: &[ContainerVersion],
        field: &Field,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?

        if let Some(added) = &self.added {
            if !versions.iter().any(|v| v.inner == *added.since) {
                return Err(
                    Error::custom("field action `added` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident.as_ref().expect("internal: field must have name").span()
                ));
            }
        }

        for rename in &self.renamed {
            if !versions.iter().any(|v| v.inner == *rename.since) {
                return Err(Error::custom("field action `renamed` uses version which was not declared via #[versioned(version)]"));
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !versions.iter().any(|v| v.inner == *deprecated.since) {
                return Err(Error::custom("field action `deprecated` uses version which was not declared via #[versioned(version)]"));
            }
        }

        Ok(())
    }
}
