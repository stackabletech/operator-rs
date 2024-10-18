use darling::{util::SpannedValue, Error, FromMeta};
use k8s_version::Version;
use proc_macro2::Span;
use syn::{spanned::Spanned, Attribute, Ident, Path, Type};

use crate::{
    attrs::common::StandaloneContainerAttributes,
    codegen::common::Attributes,
    consts::{DEPRECATED_FIELD_PREFIX, DEPRECATED_VARIANT_PREFIX},
};

/// This trait helps to unify attribute validation for both field and variant
/// attributes.
///
/// This trait is implemented using a blanket implementation on types
/// `T: Attributes`. The [`Attributes`] trait allows access to the common
/// attributes shared across field and variant attributes.
pub(crate) trait ValidateVersions<I>
where
    I: Spanned,
{
    /// Validates that each field or variant action version is present in the
    /// declared container versions.
    fn validate_versions(
        &self,
        container_attrs: &StandaloneContainerAttributes,
        item: &I,
    ) -> Result<(), darling::Error>;
}

impl<I, T> ValidateVersions<I> for T
where
    T: Attributes,
    I: Spanned,
{
    fn validate_versions(
        &self,
        container_attrs: &StandaloneContainerAttributes,
        item: &I,
    ) -> Result<(), darling::Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?

        let mut errors = Error::accumulator();

        if let Some(added) = &self.common_attributes().added {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *added.since)
            {
                errors.push(Error::custom(
                   "variant action `added` uses version which was not declared via #[versioned(version)]")
                   .with_span(item)
               );
            }
        }

        for change in &*self.common_attributes().changes {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *change.since)
            {
                errors.push(
                   Error::custom("variant action `changed` uses version which was not declared via #[versioned(version)]")
                   .with_span(item)
               );
            }
        }

        if let Some(deprecated) = &self.common_attributes().deprecated {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *deprecated.since)
            {
                errors.push(Error::custom(
                   "variant action `deprecated` uses version which was not declared via #[versioned(version)]")
                   .with_span(item)
               );
            }
        }

        errors.finish()?;
        Ok(())
    }
}

// NOTE (@Techassi): It might be possible (but is it required) to move this
// functionality into a shared trait, which knows what type of item 'Self' is.

/// This enum is used to run different validation based on the type of item.
#[derive(Debug, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum ItemType {
    Field,
    Variant,
}

// TODO (@Techassi): Shower thought: Track actions as a Vec of an ActionAttribute
// enum and implement Ord on it. This would allow us to order the items by type
// of action (added < changed < deprecated) as well as sort changed action to
// each other by specified version (which already implements Ord)

/// These attributes are meant to be used in super structs, which add
/// [`Field`](syn::Field) or [`Variant`](syn::Variant) specific attributes via
/// darling's flatten feature. This struct only provides shared attributes.
///
/// ### Shared Item Rules
///
/// - An item can only ever be added once at most. An item not marked as 'added'
///   is part of the container in every version until changed or deprecated.
/// - An item can be changed many times. That's why changes are stored in a
///   [`Vec`].
/// - An item can only be deprecated once. A field or variant not marked as
///   'deprecated' will be included up until the latest version.
#[derive(Debug, FromMeta)]
pub(crate) struct ItemAttributes {
    /// This parses the `added` attribute on items (fields or variants). It can
    /// only be present at most once.
    pub(crate) added: Option<AddedAttributes>,

    /// This parses the `changed` attribute on items (fields or variants). It
    /// can be present 0..n times.
    #[darling(multiple, rename = "changed")]
    pub(crate) changes: Vec<ChangedAttributes>,

    /// This parses the `deprecated` attribute on items (fields or variants). It
    /// can only be present at most once.
    pub(crate) deprecated: Option<DeprecatedAttributes>,
}

impl ItemAttributes {
    pub(crate) fn validate(
        &self,
        item_ident: &Ident,
        item_type: &ItemType,
        item_attrs: &Vec<Attribute>,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): This associated function is NOT called by darling's
        // and_then attribute, but instead by the wrapper, FieldAttributes and
        // VariantAttributes.

        let mut errors = Error::accumulator();

        // Semantic validation
        errors.handle(self.validate_action_combinations(item_ident, item_type));
        errors.handle(self.validate_action_order(item_ident, item_type));
        errors.handle(self.validate_item_name(item_ident, item_type));
        errors.handle(self.validate_changed_item_name(item_type));
        errors.handle(self.validate_item_attributes(item_attrs));

        // TODO (@Techassi): Add hint if a field or variant is added in the
        // first version that it might be clever to remove the 'added'
        // attribute.

        errors.finish()?;

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that each item uses a valid combination of actions.
    /// Invalid combinations are:
    ///
    /// - `added` and `deprecated` using the same version: A field or variant
    ///   cannot be marked as added in a particular version and then marked as
    ///   deprecated immediately after. Fields and variants must be included for
    ///   at least one version before being marked deprecated.
    /// - `added` and `changed` using the same version: The same reasoning from
    ///   above applies here as well. Fields and variants must be included for
    ///   at least one version before being changed.
    /// - `changed` and `deprecated` using the same version: Again, the same
    ///   rules from above apply here as well.
    fn validate_action_combinations(
        &self,
        item_ident: &Ident,
        item_type: &ItemType,
    ) -> Result<(), Error> {
        match (&self.added, &self.changes, &self.deprecated) {
            (Some(added), _, Some(deprecated)) if *added.since == *deprecated.since => {
                Err(Error::custom(format!(
                    "{item_type} cannot be marked as `added` and `deprecated` in the same version"
                ))
                .with_span(item_ident))
            }
            (Some(added), changed, _) if changed.iter().any(|r| *r.since == *added.since) => {
                Err(Error::custom(format!(
                    "{item_type} cannot be marked as `added` and `changed` in the same version"
                ))
                .with_span(item_ident))
            }
            (_, changed, Some(deprecated))
                if changed.iter().any(|r| *r.since == *deprecated.since) =>
            {
                Err(Error::custom(
                    format!("{item_type} cannot be marked as `deprecated` and `changed` in the same version"),
                )
                .with_span(item_ident))
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
    /// - All `changed` actions must use a greater version than `added` but a
    ///   lesser version than `deprecated`.
    fn validate_action_order(&self, item_ident: &Ident, item_type: &ItemType) -> Result<(), Error> {
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        // First, validate that the added version is less than the deprecated
        // version.
        // NOTE (@Techassi): Is this already covered by the code below?
        if let (Some(added_version), Some(deprecated_version)) = (added_version, deprecated_version)
        {
            if added_version > deprecated_version {
                return Err(Error::custom(format!(
                    "{item_type} was marked as `added` in version `{added_version}` while being marked as `deprecated` in an earlier version `{deprecated_version}`"
                )).with_span(item_ident));
            }
        }

        // Now, iterate over all changes and ensure that their versions are
        // between the added and deprecated version.
        if !self.changes.iter().all(|r| {
            added_version.map_or(true, |a| a < *r.since)
                && deprecated_version.map_or(true, |d| d > *r.since)
        }) {
            return Err(Error::custom(
                "all changes must use versions higher than `added` and lower than `deprecated`",
            )
            .with_span(item_ident));
        }

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that items use correct names depending on attached
    /// actions.
    ///
    /// The following naming rules apply:
    ///
    /// - Fields or variants marked as deprecated need to include the
    ///   deprecation prefix in their name. The prefix must not be included for
    ///   fields or variants which are not deprecated.
    fn validate_item_name(&self, item_ident: &Ident, item_type: &ItemType) -> Result<(), Error> {
        let prefix = match item_type {
            ItemType::Field => DEPRECATED_FIELD_PREFIX,
            ItemType::Variant => DEPRECATED_VARIANT_PREFIX,
        };

        let starts_with_deprecated = item_ident.to_string().starts_with(prefix);

        if self.deprecated.is_some() && !starts_with_deprecated {
            return Err(Error::custom(
                format!("{item_type} was marked as `deprecated` and thus must include the `{prefix}` prefix in its name")
            ).with_span(item_ident));
        }

        if self.deprecated.is_none() && starts_with_deprecated {
            return Err(Error::custom(
                format!("{item_type} includes the `{prefix}` prefix in its name but is not marked as `deprecated`")
            ).with_span(item_ident));
        }

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that disallowed item attributes are not used.
    ///
    /// The following naming rules apply:
    ///
    /// - `deprecated` must not be set on items. Instead, use the `deprecated()`
    ///   action of the `#[versioned()]` macro.
    fn validate_item_attributes(&self, item_attrs: &Vec<Attribute>) -> Result<(), Error> {
        for attr in item_attrs {
            for segment in &attr.path().segments {
                if segment.ident == "deprecated" {
                    return Err(Error::custom("deprecation must be done using #[versioned(deprecated(since = \"VERSION\"))]")
                        .with_span(&attr.span()));
                }
            }
        }
        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that parameters provided to the `changed` actions are
    /// valid.
    fn validate_changed_item_name(&self, item_type: &ItemType) -> Result<(), Error> {
        let prefix = match item_type {
            ItemType::Field => DEPRECATED_FIELD_PREFIX,
            ItemType::Variant => DEPRECATED_VARIANT_PREFIX,
        };

        let mut errors = Error::accumulator();

        // This ensures that `from_name` doesn't include the deprecation prefix.
        for change in &self.changes {
            if let Some(from_name) = change.from_name.as_ref() {
                if from_name.starts_with(prefix) {
                    errors.push(
                        Error::custom(format!(
                            "the previous {item_type} name must not start with the deprecation prefix"
                        ))
                        .with_span(&from_name.span()),
                    );
                }
            }
        }

        errors.finish()
    }
}

// TODO (@Techassi): Add validation for when default_fn is "" (empty path).
/// For the added() action
///
/// Example usage:
/// - `added(since = "...")`
/// - `added(since = "...", default_fn = "custom_fn")`
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    pub(crate) since: SpannedValue<Version>,

    #[darling(rename = "default", default = "default_default_fn")]
    pub(crate) default_fn: SpannedValue<Path>,
}

fn default_default_fn() -> SpannedValue<Path> {
    SpannedValue::new(
        syn::parse_str("::std::default::Default::default")
            .expect("internal error: path must parse"),
        Span::call_site(),
    )
}

// TODO (@Techassi): Add validation for when from_name AND from_type are both
// none => is this action needed in the first place?
// TODO (@Techassi): Add validation that the from_name mustn't include the
// deprecated prefix.
/// For the changed() action
///
/// Example usage:
/// - `changed(since = "...", from_name = "...")`
/// - `changed(since = "...", from_name = "..." from_type="...")`
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct ChangedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) from_name: Option<SpannedValue<String>>,
    pub(crate) from_type: Option<SpannedValue<Type>>,
}

/// For the deprecated() action
///
/// Example usage:
/// - `deprecated(since = "...")`
/// - `deprecated(since = "...", note = "...")`
#[derive(Clone, Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) note: Option<SpannedValue<String>>,
}
