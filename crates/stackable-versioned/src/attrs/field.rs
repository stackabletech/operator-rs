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
    fn validate(self) -> Result<Self, Error> {
        self.validate_action_combinations()?;
        // self.validate_rename_order()?;

        // TODO (@Techassi): Add validation for renames so that renamed fields
        // match up and form a continous chain (eg. foo -> bar -> baz).

        // TODO (@Techassi): Add hint if a field is added in the first version
        // that it might be clever to remove the 'added' attribute.

        self.validate_action_order()?;
        self.validate_field_name()?;

        Ok(self)
    }

    fn validate_action_combinations(&self) -> Result<(), Error> {
        match (&self.added, &self.renames, &self.deprecated) {
            // The derive macro prohibits the use of the 'added' and 'deprecated'
            // field action using the same version. This is because it doesn't
            // make sense to add a field and immediatly mark that field as
            // deprecated in the same version. Instead, fields should be
            // deprecated at least one version later.
            (Some(added), _, Some(deprecated)) if *added.since == *deprecated.since => {
                Err(Error::custom(
                    "field cannot be marked as `added` and `deprecated` in the same version",
                )
                .with_span(&self.ident.span()))
            }
            (Some(added), renamed, _) if renamed.iter().any(|r| *r.since == *added.since) => {
                Err(Error::custom(
                    "field cannot be marked as `added` and `renamed` in the same version",
                )
                .with_span(&self.ident.span()))
            }
            (_, renamed, Some(deprecated))
                if renamed.iter().any(|r| *r.since == *deprecated.since) =>
            {
                Err(Error::custom(
                    "field cannot be marked as `deprecated` and `renamed` in the same version",
                )
                .with_span(&self.ident.span()))
            }
            _ => Ok(()),
        }
    }

    fn validate_action_order(&self) -> Result<(), Error> {
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        // First, validate that the added version is less than the deprecated
        // version.
        if let (Some(added_version), Some(deprecated_version)) = (added_version, deprecated_version)
        {
            if added_version >= deprecated_version {
                return Err(Error::custom(format!(
                    "field was marked as `added` in version `{}` while being marked as `deprecated` in an earlier version `{}`",
                    added_version,
                    deprecated_version
                )).with_span(&self.ident.span()));
            }
        }

        // Now, iterate over all renames and ensure that their versions are
        // between the added and deprecated version.
        if !self.renames.iter().all(|r| {
            added_version.map_or(true, |a| a < *r.since)
                && deprecated_version.map_or(true, |d| d > *r.since)
        }) {
            return Err(Error::custom(
                "all renames must use versions higher than `added` and lower than `deprecated`",
            )
            .with_span(&self.ident.span()));
        }

        Ok(())
    }

    fn validate_field_name(&self) -> Result<(), Error> {
        let starts_with = self
            .ident
            .as_ref()
            .unwrap()
            .to_string()
            .starts_with(DEPRECATED_PREFIX);

        if self.deprecated.is_some() && !starts_with {
            return Err(Error::custom(
                "field was marked as `deprecated` and thus must include the `deprecated_` prefix in its name"
            ).with_span(&self.ident.span()));
        } else if starts_with {
            return Err(Error::custom(
                "field includes the `deprecated_` prefix in its name but is not marked as `deprecated`"
            ).with_span(&self.ident.span()));
        }

        Ok(())
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
