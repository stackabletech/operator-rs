use darling::{util::SpannedValue, Error, FromField, FromMeta};
use k8s_version::Version;
use proc_macro2::Span;
use syn::{Field, Ident, Path};

use crate::{attrs::container::ContainerAttributes, consts::DEPRECATED_PREFIX};

/// This struct describes all available field attributes, as well as the field
/// name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`FieldAttributes::validate_versions`]
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

    #[darling(rename = "default", default = "default_default_fn")]
    pub(crate) default_fn: SpannedValue<Path>,
}

fn default_default_fn() -> SpannedValue<Path> {
    SpannedValue::new(
        syn::parse_str("std::default::Default::default").unwrap(),
        Span::call_site(),
    )
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) from: SpannedValue<String>,
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) note: SpannedValue<String>,
}

impl FieldAttributes {
    /// This associated function is called by darling (see and_then attribute)
    /// after it successfully parsed the attribute. This allows custom
    /// validation of the attribute which extends the validation already in
    /// place by darling.
    ///
    /// Internally, it calls out to other specialized validation functions.
    fn validate(self) -> Result<Self, Error> {
        let mut errors = Error::accumulator();

        // Semantic validation
        errors.handle(self.validate_action_combinations());
        errors.handle(self.validate_action_order());
        errors.handle(self.validate_field_name());

        // Code quality validation
        errors.handle(self.validate_deprecated_options());

        // TODO (@Techassi): Add validation for renames so that renamed fields
        // match up and form a continous chain (eg. foo -> bar -> baz).

        // TODO (@Techassi): Add hint if a field is added in the first version
        // that it might be clever to remove the 'added' attribute.

        errors.finish()?;
        Ok(self)
    }

    /// This associated function is called by the top-level validation function
    /// and validates that each field uses a valid combination of actions.
    /// Invalid combinations are:
    ///
    /// - `added` and `deprecated` using the same version: A field cannot be
    ///   marked as added in a particular version and then marked as deprecated
    ///   immediately after. Fields must be included for at least one version
    ///   before being marked deprecated.
    /// - `added` and `renamed` using the same version: The same reasoning from
    ///   above applies here as well. Fields must be included for at least one
    ///   version before being renamed.
    /// - `renamed` and `deprecated` using the same version: Again, the same
    ///   rules from above apply here as well.
    fn validate_action_combinations(&self) -> Result<(), Error> {
        match (&self.added, &self.renames, &self.deprecated) {
            (Some(added), _, Some(deprecated)) if *added.since == *deprecated.since => {
                Err(Error::custom(
                    "field cannot be marked as `added` and `deprecated` in the same version",
                )
                .with_span(&self.ident))
            }
            (Some(added), renamed, _) if renamed.iter().any(|r| *r.since == *added.since) => {
                Err(Error::custom(
                    "field cannot be marked as `added` and `renamed` in the same version",
                )
                .with_span(&self.ident))
            }
            (_, renamed, Some(deprecated))
                if renamed.iter().any(|r| *r.since == *deprecated.since) =>
            {
                Err(Error::custom(
                    "field cannot be marked as `deprecated` and `renamed` in the same version",
                )
                .with_span(&self.ident))
            }
            _ => Ok(()),
        }
    }

    /// This associated function is called by the top-level validation function
    /// and validates that actions use a chronologically sound chain of
    /// versions.
    ///
    /// The following rules apply:
    ///
    /// - `deprecated` must use a greater version than `added`: This function
    ///   ensures that these versions are chronologically sound, that means,
    ///   that the version of the deprecated action must be greater than the
    ///   version of the added action.
    /// - All `renamed` actions must use a greater version than `added` but a
    ///   lesser version than `deprecated`.
    fn validate_action_order(&self) -> Result<(), Error> {
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        // First, validate that the added version is less than the deprecated
        // version.
        // NOTE (@Techassi): Is this already covered by the code below?
        if let (Some(added_version), Some(deprecated_version)) = (added_version, deprecated_version)
        {
            if added_version >= deprecated_version {
                return Err(Error::custom(format!(
                    "field was marked as `added` in version `{added_version}` while being marked as `deprecated` in an earlier version `{deprecated_version}`"
                )).with_span(&self.ident));
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
            .with_span(&self.ident));
        }

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that fields use correct names depending on attached
    /// actions.
    ///
    /// The following naming rules apply:
    ///
    /// - Fields marked as deprecated need to include the 'deprecated_' prefix
    ///   in their name. The prefix must not be included for fields which are
    ///   not deprecated.
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
            ).with_span(&self.ident));
        }

        if self.deprecated.is_none() && starts_with {
            return Err(Error::custom(
                "field includes the `deprecated_` prefix in its name but is not marked as `deprecated`"
            ).with_span(&self.ident));
        }

        Ok(())
    }

    fn validate_deprecated_options(&self) -> Result<(), Error> {
        // TODO (@Techassi): Make the field 'note' optional, because in the
        // future, the macro will generate parts of the deprecation note
        // automatically. The user-provided note will then be appended to the
        // auto-generated one.

        if let Some(deprecated) = &self.deprecated {
            if deprecated.note.is_empty() {
                return Err(Error::custom("deprecation note must not be empty")
                    .with_span(&deprecated.note.span()));
            }
        }

        Ok(())
    }

    /// Validates that each field action version is present in the declared
    /// container versions.
    pub(crate) fn validate_versions(
        &self,
        container_attrs: &ContainerAttributes,
        field: &Field,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?
        let mut errors = Error::accumulator();

        if let Some(added) = &self.added {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *added.since)
            {
                errors.push(Error::custom(
                    "field action `added` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident)
                );
            }
        }

        for rename in &self.renames {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *rename.since)
            {
                errors.push(
                    Error::custom("field action `renamed` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident)
                );
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *deprecated.since)
            {
                errors.push(Error::custom(
                    "field action `deprecated` uses version which was not declared via #[versioned(version)]")
                    .with_span(&field.ident)
                );
            }
        }

        errors.finish()?;
        Ok(())
    }
}
