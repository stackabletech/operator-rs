//! Fragments are partially validated parts of a product configuration. For example, mandatory values may be missing.
//! Fragments may be [`validate`]d and turned into their ["full"](`FromFragment`) type.
//!
//! Fragment types are typically generated using the [`#[derive(Fragment)]`](`derive@Fragment`) macro.
use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Display, Write},
    hash::Hash,
};

use super::merge::Atomic;

#[cfg(doc)]
use super::merge::Merge;
#[cfg(doc)]
use crate::role_utils::{Role, RoleGroup};

use k8s_openapi::api::core::v1::PodTemplateSpec;
use snafu::Snafu;

pub const FILE_HEADER_KEY: &str = "EXPERIMENTAL_FILE_HEADER";
pub const FILE_FOOTER_KEY: &str = "EXPERIMENTAL_FILE_FOOTER";

pub use stackable_operator_derive::Fragment;

/// Contains context used for generating validation errors
///
/// Constructed internally in [`validate`]
pub struct Validator<'a> {
    ident: Option<&'a dyn Display>,
    parent: Option<&'a Validator<'a>>,
}

impl<'a> Validator<'a> {
    /// Creates a `Validator` for a subfield of the current object
    pub fn field<'b>(&'b self, ident: &'b dyn Display) -> Validator<'b> {
        Validator {
            ident: Some(ident),
            parent: Some(self),
        }
    }

    fn error_problem(self, problem: ValidationProblem) -> ValidationError {
        let mut idents = Vec::new();
        let mut curr = Some(&self);
        while let Some(curr_some) = curr {
            if let Some(ident) = curr_some.ident {
                idents.push(ident.to_string());
            }
            curr = curr_some.parent;
        }
        ValidationError {
            path: FieldPath { idents },
            problem,
        }
    }

    /// Returns an error indicating that the `Validator` refers to a required field that is currently not provided
    pub fn error_required(self) -> ValidationError {
        self.error_problem(ValidationProblem::FieldRequired)
    }
}

#[derive(Debug, PartialEq)]
struct FieldPath {
    idents: Vec<String>,
}
impl Display for FieldPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, ident) in self.idents.iter().rev().enumerate() {
            if i > 0 {
                f.write_char('.')?;
            }
            f.write_str(ident)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Snafu)]
#[snafu(display("failed to validate {path}"))]
/// An error that occurred when validating an object.
///
/// It is constructed by calling one of the `error_*` methods on [`Validator`], such as [`Validator::error_required`].
pub struct ValidationError {
    path: FieldPath,
    #[snafu(source)]
    problem: ValidationProblem,
}
/// A problem that was discovered during validation, with no additional context.
#[derive(Debug, PartialEq, Snafu)]
enum ValidationProblem {
    #[snafu(display("field is required"))]
    FieldRequired,
}

/// A type that can be constructed by validating a "fragment" type.
///
/// This is intended to be used together with [`Merge`], such that fragments are deserialized from multiple sources
/// (for example: the [`RoleGroup`] and [`Role`] levels of a ProductCluster object), and then validated into the type implementing
/// `FromFragment`.
///
/// It is recommended to use [`RoleGroup::validate_config`] to both merge and validate product [`RoleGroup`] configurations. For other use cases,
/// [`validate`] can be used on the already-merged configuration.
///
/// This will typically be derived using the [`Fragment`] macro, rather than implemented manually.
pub trait FromFragment: Sized {
    /// The fragment type of `Self`.
    ///
    /// For [`Atomic`] types this should be [`Option`](`Option<Self>`).
    ///
    /// For complex structs, this should be a variant of `Self` where each field is replaced by its respective `Fragment` type. This can be derived using
    /// [`Fragment`].
    type Fragment;
    /// A variant of [`Self::Fragment`] that is used when the container already provides a to indicate that a value is optional.
    ///
    /// For example, there's no use marking a value as [`Option`]al again if the value is already contained in an `Option`.
    ///
    /// For [`Atomic`]s this will typically be `Self`. For complex structs this will typically be [`Self::Fragment`].
    type RequiredFragment: Into<Self::Fragment>;

    /// Try to validate a [`Self::Fragment`] into `Self`.
    ///
    /// `validator` contains additional error reporting context, such as the path to the field from the root fragment. It is created by
    /// [`validate`].
    fn from_fragment(
        fragment: Self::Fragment,
        validator: Validator,
    ) -> Result<Self, ValidationError>;
}
impl<T: Atomic> FromFragment for T {
    type Fragment = Option<T>;
    type RequiredFragment = T;

    fn from_fragment(
        fragment: Self::Fragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        fragment.ok_or_else(|| validator.error_required())
    }
}
impl<K, V: FromFragment> FromFragment for HashMap<K, V>
where
    K: Eq + Hash + Display,
{
    type Fragment = HashMap<K, V::RequiredFragment>;
    type RequiredFragment = HashMap<K, V::RequiredFragment>;

    fn from_fragment(
        fragment: Self::Fragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        fragment
            .into_iter()
            .map(|(k, v)| {
                let validator = validator.field(&k);
                let v = V::from_fragment(v.into(), validator)?;
                Ok((k, v))
            })
            .collect()
    }
}
impl<K, V: FromFragment> FromFragment for BTreeMap<K, V>
where
    K: Eq + Ord + Display,
{
    type Fragment = BTreeMap<K, V::RequiredFragment>;
    type RequiredFragment = BTreeMap<K, V::RequiredFragment>;

    fn from_fragment(
        fragment: Self::Fragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        fragment
            .into_iter()
            .map(|(k, v)| {
                let validator = validator.field(&k);
                let v = V::from_fragment(v.into(), validator)?;
                Ok((k, v))
            })
            .collect()
    }
}
impl<T: FromFragment> FromFragment for Option<T> {
    type Fragment = Option<T::RequiredFragment>;
    type RequiredFragment = Option<T::RequiredFragment>;

    fn from_fragment(
        fragment: Self::Fragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        if let Some(fragment) = fragment {
            T::from_fragment(fragment.into(), validator).map(Some)
        } else {
            Ok(None)
        }
    }
}
impl FromFragment for PodTemplateSpec {
    type Fragment = PodTemplateSpec;
    type RequiredFragment = PodTemplateSpec;

    fn from_fragment(
        fragment: Self::Fragment,
        _validator: Validator,
    ) -> Result<Self, ValidationError> {
        Ok(fragment)
    }
}

/// Validates a [`Fragment`](`FromFragment::Fragment`), and turns it into its corresponding [`FromFragment`] type if successful.
///
/// When validating a [`RoleGroup`]'s configuration, consider using [`RoleGroup::validate_config`] instead.
pub fn validate<T: FromFragment>(fragment: T::Fragment) -> Result<T, ValidationError> {
    T::from_fragment(
        fragment,
        Validator {
            ident: None,
            parent: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use schemars::{schema_for, JsonSchema};

    use super::{validate, Fragment};

    #[derive(Fragment, Debug, PartialEq, Eq)]
    #[fragment(path_overrides(fragment = "super"))]
    #[fragment_attrs(derive(Debug))]
    struct Empty {}

    #[derive(Fragment, Debug, PartialEq, Eq, JsonSchema)]
    #[fragment(path_overrides(fragment = "super"))]
    #[fragment_attrs(derive(Default, JsonSchema))]
    /// This is an awesome struct with fields
    struct WithFields {
        /// This field contains the name
        name: String,
        replicas: u8,
        overhead: u8,
        tag: Option<String>,
    }

    #[derive(Fragment, Debug, PartialEq, Eq)]
    #[fragment(path_overrides(fragment = "super"))]
    struct Nested {
        required: WithFields,
        optional: Option<WithFields>,
    }

    #[derive(Fragment, Debug, PartialEq, Eq)]
    #[fragment(path_overrides(fragment = "super"))]
    struct GenericNested<T: super::FromFragment> {
        required: T,
        optional: Option<T>,
    }

    #[test]
    fn validate_empty() {
        assert_eq!(validate::<Empty>(EmptyFragment {}).unwrap(), Empty {});
    }

    #[test]
    fn validate_basics() {
        assert_eq!(
            validate::<WithFields>(WithFieldsFragment {
                name: Some("foo".to_string()),
                replicas: Some(23),
                overhead: Some(24),
                tag: Some("bar".to_string()),
            })
            .unwrap(),
            WithFields {
                name: "foo".to_string(),
                replicas: 23,
                overhead: 24,
                tag: Some("bar".to_string()),
            }
        );

        let err = validate::<WithFields>(WithFieldsFragment {
            name: None,
            replicas: None,
            overhead: None,
            tag: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validate_nested() {
        let err = validate::<Nested>(NestedFragment {
            required: WithFieldsFragment::default(),
            optional: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("required.name"));

        // optional complex fields should still be treated as optional if not provided
        let nested = validate::<Nested>(NestedFragment {
            required: WithFieldsFragment {
                name: Some("name".to_string()),
                replicas: Some(2),
                overhead: Some(3),
                ..Default::default()
            },
            optional: None,
        })
        .unwrap();
        assert_eq!(nested.optional, None);
    }

    #[test]
    fn validate_struct_description() {
        let schema = schema_for!(WithFieldsFragment);

        let struct_description = schema.schema.metadata.unwrap().description;
        assert_eq!(
            struct_description,
            Some("This is an awesome struct with fields".to_string())
        );
    }

    #[test]
    fn validate_field_description() {
        let schema = schema_for!(WithFieldsFragment);
        let field_schema = schema
            .schema
            .object
            .unwrap()
            .as_ref()
            .properties
            .get("name")
            .unwrap()
            .clone()
            .into_object();

        assert_eq!(
            field_schema.metadata.unwrap().description,
            Some("This field contains the name".to_string())
        );
    }
}
