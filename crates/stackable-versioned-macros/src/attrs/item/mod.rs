use std::{collections::BTreeMap, ops::Deref};

use darling::{Error, FromMeta, Result, util::SpannedValue};
use k8s_version::Version;
use proc_macro2::Span;
use quote::format_ident;
use syn::{Attribute, Path, Type, spanned::Spanned};

use crate::{
    codegen::{ItemStatus, VersionDefinition},
    utils::ItemIdentExt,
};

mod field;
pub(crate) use field::*;

mod variant;
pub(crate) use variant::*;

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
pub(crate) struct CommonItemAttributes {
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

// This impl block ONLY contains validation. The main entrypoint is the associated 'validate'
// function. In addition to validate functions which are called directly during darling's parsing,
// it contains functions which can only be called after the initial parsing and validation because
// they need additional context, namely the list of versions defined on the container or module.
impl CommonItemAttributes {
    pub(crate) fn validate(
        &self,
        item_ident: impl ItemIdentExt,
        item_attrs: &[Attribute],
    ) -> Result<()> {
        let mut errors = Error::accumulator();

        errors.handle(self.validate_action_combinations(&item_ident));
        errors.handle(self.validate_action_order(&item_ident));
        errors.handle(self.validate_item_name(&item_ident));
        errors.handle(self.validate_added_action());
        errors.handle(self.validate_changed_action(&item_ident));
        errors.handle(self.validate_item_attributes(item_attrs));

        errors.finish()
    }

    pub(crate) fn validate_versions(&self, versions: &[VersionDefinition]) -> Result<()> {
        let mut errors = Error::accumulator();

        if let Some(added) = &self.added {
            if !versions.iter().any(|v| v.inner == *added.since) {
                errors.push(Error::custom(
                    "the `added` action uses a version which is not declared via `#[versioned(version)]`",
                ).with_span(&added.since.span()));
            }
        }

        for change in &self.changes {
            if !versions.iter().any(|v| v.inner == *change.since) {
                errors.push(Error::custom(
                    "the `changed` action uses a version which is not declared via `#[versioned(version)]`"
                ).with_span(&change.since.span()));
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !versions.iter().any(|v| v.inner == *deprecated.since) {
                errors.push(Error::custom(
                    "the `deprecated` action uses a version which is not declared via `#[versioned(version)]`",
                ).with_span(&deprecated.since.span()));
            }
        }

        errors.finish()
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
    fn validate_action_combinations(&self, item_ident: &impl ItemIdentExt) -> Result<()> {
        match (&self.added, &self.changes, &self.deprecated) {
            (Some(added), _, Some(deprecated)) if *added.since == *deprecated.since => Err(
                Error::custom("cannot be marked as `added` and `deprecated` in the same version")
                    .with_span(item_ident),
            ),
            (Some(added), changed, _) if changed.iter().any(|r| *r.since == *added.since) => Err(
                Error::custom("cannot be marked as `added` and `changed` in the same version")
                    .with_span(item_ident),
            ),
            (_, changed, Some(deprecated))
                if changed.iter().any(|r| *r.since == *deprecated.since) =>
            {
                Err(Error::custom(
                    "cannot be marked as `deprecated` and `changed` in the same version",
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
    fn validate_action_order(&self, item_ident: &impl ItemIdentExt) -> Result<()> {
        let added_version = self.added.as_ref().map(|a| *a.since);
        let deprecated_version = self.deprecated.as_ref().map(|d| *d.since);

        // First, validate that the added version is less than the deprecated
        // version.
        // NOTE (@Techassi): Is this already covered by the code below?
        if let (Some(added_version), Some(deprecated_version)) = (added_version, deprecated_version)
        {
            if added_version > deprecated_version {
                return Err(Error::custom(format!(
                    "cannot marked as `added` in version `{added_version}` while being marked as `deprecated` in an earlier version `{deprecated_version}`"
                )).with_span(item_ident));
            }
        }

        // Now, iterate over all changes and ensure that their versions are
        // between the added and deprecated version.
        if !self.changes.iter().all(|r| {
            added_version.is_none_or(|a| a < *r.since)
                && deprecated_version.is_none_or(|d| d > *r.since)
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
    fn validate_item_name(&self, item_ident: &impl ItemIdentExt) -> Result<()> {
        let starts_with_deprecated = item_ident.starts_with_deprecated_prefix();

        if self.deprecated.is_some() && !starts_with_deprecated {
            return Err(Error::custom(format!(
                "marked as `deprecated` and thus must include the `{deprecated_prefix}` prefix",
                deprecated_prefix = item_ident.deprecated_prefix()
            ))
            .with_span(item_ident));
        }

        if self.deprecated.is_none() && starts_with_deprecated {
            return Err(Error::custom(
                format!("not marked as `deprecated` and thus must not include the `{deprecated_prefix}` prefix", deprecated_prefix = item_ident.deprecated_prefix())
            ).with_span(item_ident));
        }

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that parameters provided to the `added` actions are
    /// valid.
    fn validate_added_action(&self) -> Result<()> {
        // NOTE (@Techassi): Can the path actually be empty?
        if let Some(added) = &self.added {
            if added.default_fn.segments.is_empty() {
                return Err(Error::custom("`default_fn` cannot be empty")
                    .with_span(&added.default_fn.span()));
            }
        }

        Ok(())
    }

    /// This associated function is called by the top-level validation function
    /// and validates that parameters provided to the `changed` actions are
    /// valid.
    fn validate_changed_action(&self, item_ident: &impl ItemIdentExt) -> Result<()> {
        let mut errors = Error::accumulator();

        // This ensures that `from_name` doesn't include the deprecation prefix.
        for change in &self.changes {
            if let Some(from_name) = change.from_name.as_ref() {
                if from_name.starts_with(item_ident.deprecated_prefix()) {
                    errors.push(
                        Error::custom(
                            "the previous name must not start with the deprecation prefix",
                        )
                        .with_span(&from_name.span()),
                    );
                }
            }

            if change.from_type.is_none() {
                // The upgrade_with argument only makes sense to use when the
                // type changed
                if let Some(upgrade_func) = change.upgrade_with.as_ref() {
                    errors.push(
                        Error::custom(
                            "the `upgrade_with` argument must be used in combination with `from_type`",
                        )
                        .with_span(&upgrade_func.span()),
                    );
                }

                // The downgrade_with argument only makes sense to use when the
                // type changed
                if let Some(downgrade_func) = change.downgrade_with.as_ref() {
                    errors.push(
                        Error::custom(
                            "the `downgrade_with` argument must be used in combination with `from_type`",
                        )
                        .with_span(&downgrade_func.span()),
                    );
                }
            }
        }

        errors.finish()
    }

    /// This associated function is called by the top-level validation function
    /// and validates that disallowed item attributes are not used.
    ///
    /// The following naming rules apply:
    ///
    /// - `deprecated` must not be set on items. Instead, use the `deprecated()`
    ///   action of the `#[versioned()]` macro.
    fn validate_item_attributes(&self, item_attrs: &[Attribute]) -> Result<()> {
        for attr in item_attrs {
            for segment in &attr.path().segments {
                if segment.ident == "deprecated" {
                    return Err(Error::custom("deprecation must be done using `#[versioned(deprecated(since = \"VERSION\"))]`")
                        .with_span(&attr.span()));
                }
            }
        }
        Ok(())
    }
}

impl CommonItemAttributes {
    pub(crate) fn into_changeset(
        self,
        ident: &impl ItemIdentExt,
        ty: Type,
    ) -> Option<BTreeMap<Version, ItemStatus>> {
        // TODO (@Techassi): Use Change instead of ItemStatus
        if let Some(deprecated) = self.deprecated {
            let deprecated_ident = ident.deref();

            // When the item is deprecated, any change which occurred beforehand
            // requires access to the item ident to infer the item ident for
            // the latest change.
            let mut ident = ident.as_cleaned_ident();
            let mut ty = ty;

            let mut actions = BTreeMap::new();

            actions.insert(*deprecated.since, ItemStatus::Deprecation {
                previous_ident: ident.clone(),
                ident: deprecated_ident.clone(),
                note: deprecated.note.as_deref().cloned(),
            });

            for change in self.changes.iter().rev() {
                let from_ident = if let Some(from) = change.from_name.as_deref() {
                    format_ident!("{from}").into()
                } else {
                    ident.clone()
                };

                // TODO (@Techassi): This is an awful lot of cloning, can we get
                // rid of it?
                let from_ty = change
                    .from_type
                    .as_ref()
                    .map(|sv| sv.deref().clone())
                    .unwrap_or(ty.clone());

                actions.insert(*change.since, ItemStatus::Change {
                    downgrade_with: change.downgrade_with.as_deref().cloned(),
                    upgrade_with: change.upgrade_with.as_deref().cloned(),
                    from_ident: from_ident.clone(),
                    from_type: from_ty.clone(),
                    to_ident: ident,
                    to_type: ty,
                });

                ident = from_ident;
                ty = from_ty;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = self.added {
                actions.insert(*added.since, ItemStatus::Addition {
                    default_fn: added.default_fn.deref().clone(),
                    ident,
                    ty,
                });
            }

            Some(actions)
        } else if !self.changes.is_empty() {
            let mut ident = ident.deref().clone();
            let mut ty = ty;

            let mut actions = BTreeMap::new();

            for change in self.changes.iter().rev() {
                let from_ident = if let Some(from) = change.from_name.as_deref() {
                    format_ident!("{from}").into()
                } else {
                    ident.clone()
                };

                // TODO (@Techassi): This is an awful lot of cloning, can we get
                // rid of it?
                let from_ty = change
                    .from_type
                    .as_ref()
                    .map(|sv| sv.deref().clone())
                    .unwrap_or(ty.clone());

                actions.insert(*change.since, ItemStatus::Change {
                    downgrade_with: change.downgrade_with.as_deref().cloned(),
                    upgrade_with: change.upgrade_with.as_deref().cloned(),
                    from_ident: from_ident.clone(),
                    from_type: from_ty.clone(),
                    to_ident: ident,
                    to_type: ty,
                });

                ident = from_ident;
                ty = from_ty;
            }

            // After the last iteration above (if any) we use the ident for the
            // added action if there is any.
            if let Some(added) = self.added {
                actions.insert(*added.since, ItemStatus::Addition {
                    default_fn: added.default_fn.deref().clone(),
                    ident,
                    ty,
                });
            }

            Some(actions)
        } else {
            if let Some(added) = self.added {
                let mut actions = BTreeMap::new();

                actions.insert(*added.since, ItemStatus::Addition {
                    default_fn: added.default_fn.deref().clone(),
                    ident: ident.deref().clone(),
                    ty,
                });

                return Some(actions);
            }

            None
        }
    }
}

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
/// - `changed(since = "...", from_name = "...", from_type="...")`
/// - `changed(since = "...", from_name = "...", from_type="...", convert_with = "...")`
#[derive(Clone, Debug, FromMeta)]
pub struct ChangedAttributes {
    pub since: SpannedValue<Version>,
    pub from_name: Option<SpannedValue<String>>,
    pub from_type: Option<SpannedValue<Type>>,
    pub upgrade_with: Option<SpannedValue<Path>>,
    pub downgrade_with: Option<SpannedValue<Path>>,
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
