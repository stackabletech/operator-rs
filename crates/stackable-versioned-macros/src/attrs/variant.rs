use convert_case::{Case, Casing};
use darling::{Error, FromVariant};
use syn::{Ident, Variant};

use crate::{
    attrs::common::{ContainerAttributes, ItemAttributes},
    consts::DEPRECATED_VARIANT_PREFIX,
};

#[derive(Debug, FromVariant)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = VariantAttributes::validate
)]
pub(crate) struct VariantAttributes {
    #[darling(flatten)]
    pub(crate) common: ItemAttributes,

    // The ident (automatically extracted by darling) cannot be moved into the
    // shared item attributes because for struct fields, the type is
    // `Option<Ident>`, while for enum variants, the type is `Ident`.
    pub(crate) ident: Ident,
}

impl VariantAttributes {
    // NOTE (@Techassi): Ideally, these validations should be moved to the
    // ItemAttributes impl, because common validation like action combinations
    // and action order can be validated without taking the type of attribute
    // into account (field vs variant). However, we would loose access to the
    // field / variant ident and as such, cannot display the error directly on
    // the affected field / variant. This is a significant decrease in DX.
    // See https://github.com/TedDriggs/darling/discussions/294

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
        errors.handle(self.validate_variant_name());

        // TODO (@Techassi): Add validation for renames so that renamed items
        // match up and form a continuous chain (eg. foo -> bar -> baz).

        // TODO (@Techassi): Add hint if a item is added in the first version
        // that it might be clever to remove the 'added' attribute.

        errors.finish()?;
        Ok(self)
    }

    /// This associated function is called by the top-level validation function
    /// and validates that each variant uses a valid combination of actions.
    /// Invalid combinations are:
    ///
    /// - `added` and `deprecated` using the same version: A variant cannot be
    ///   marked as added in a particular version and then marked as deprecated
    ///   immediately after. Variants must be included for at least one version
    ///   before being marked deprecated.
    /// - `added` and `renamed` using the same version: The same reasoning from
    ///   above applies here as well. Variants must be included for at least one
    ///   version before being renamed.
    /// - `renamed` and `deprecated` using the same version: Again, the same
    ///   rules from above apply here as well.
    fn validate_action_combinations(&self) -> Result<(), Error> {
        match (
            &self.common.added,
            &self.common.renames,
            &self.common.deprecated,
        ) {
            (Some(added), _, Some(deprecated)) if *added.since == *deprecated.since => {
                Err(Error::custom(
                    "variant cannot be marked as `added` and `deprecated` in the same version",
                )
                .with_span(&self.ident))
            }
            (Some(added), renamed, _) if renamed.iter().any(|r| *r.since == *added.since) => {
                Err(Error::custom(
                    "variant cannot be marked as `added` and `renamed` in the same version",
                )
                .with_span(&self.ident))
            }
            (_, renamed, Some(deprecated))
                if renamed.iter().any(|r| *r.since == *deprecated.since) =>
            {
                Err(Error::custom(
                    "variant cannot be marked as `deprecated` and `renamed` in the same version",
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
        let added_version = self.common.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.common.deprecated.as_ref().map(|d| *d.since);

        // First, validate that the added version is less than the deprecated
        // version.
        // NOTE (@Techassi): Is this already covered by the code below?
        if let (Some(added_version), Some(deprecated_version)) = (added_version, deprecated_version)
        {
            if added_version > deprecated_version {
                return Err(Error::custom(format!(
                    "variant was marked as `added` in version `{added_version}` while being marked as `deprecated` in an earlier version `{deprecated_version}`"
                )).with_span(&self.ident));
            }
        }

        // Now, iterate over all renames and ensure that their versions are
        // between the added and deprecated version.
        if !self.common.renames.iter().all(|r| {
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
    /// and validates that variants use correct names depending on attached
    /// actions.
    ///
    /// The following naming rules apply:
    ///
    /// - Variants marked as deprecated need to include the 'deprecated_' prefix
    ///   in their name. The prefix must not be included for variants which are
    ///   not deprecated.
    fn validate_variant_name(&self) -> Result<(), Error> {
        if !self
            .common
            .renames
            .iter()
            .all(|r| r.from.is_case(Case::Pascal))
        {
            return Err(Error::custom("renamed variants must use PascalCase"));
        }

        let starts_with_deprecated = self
            .ident
            .to_string()
            .starts_with(DEPRECATED_VARIANT_PREFIX);

        if self.common.deprecated.is_some() && !starts_with_deprecated {
            return Err(Error::custom(
                "variant was marked as `deprecated` and thus must include the `Deprecated` prefix in its name"
            ).with_span(&self.ident));
        }

        if self.common.deprecated.is_none() && starts_with_deprecated {
            return Err(Error::custom(
                "variant includes the `Deprecated` prefix in its name but is not marked as `deprecated`"
            ).with_span(&self.ident));
        }

        Ok(())
    }

    pub(crate) fn validate_versions(
        &self,
        container_attrs: &ContainerAttributes,
        variant: &Variant,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?
        // TODO (@Techassi): Unify this with the field impl, e.g. by introducing
        // a T: Spanned bound for the second function parameter.
        let mut errors = Error::accumulator();

        if let Some(added) = &self.common.added {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *added.since)
            {
                errors.push(Error::custom(
                   "variant action `added` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        for rename in &*self.common.renames {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *rename.since)
            {
                errors.push(
                   Error::custom("variant action `renamed` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        if let Some(deprecated) = &self.common.deprecated {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *deprecated.since)
            {
                errors.push(Error::custom(
                   "variant action `deprecated` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        errors.finish()?;
        Ok(())
    }
}
