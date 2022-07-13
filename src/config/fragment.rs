use std::fmt::{Display, Write};

pub use stackable_operator_derive::Fragment;

use super::merge::Atomic;

use snafu::Snafu;

pub struct Validator<'a> {
    ident: Option<&'a str>,
    parent: Option<&'a Validator<'a>>,
}

impl<'a> Validator<'a> {
    pub fn field<'b>(&'b self, ident: &'b str) -> Validator<'b> {
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

    pub fn error_required(self) -> ValidationError {
        self.error_problem(ValidationProblem::FieldRequired)
    }
}

#[derive(Debug)]
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

#[derive(Debug, Snafu)]
#[snafu(display("failed to validate {path}"))]
pub struct ValidationError {
    path: FieldPath,
    #[snafu(source)]
    problem: ValidationProblem,
}
#[derive(Debug, Snafu)]
enum ValidationProblem {
    #[snafu(display("field is required"))]
    FieldRequired,
}

pub trait FromFragment: Sized {
    type RequiredFragment: Into<Self::OptionalFragment>;
    type OptionalFragment;

    fn from_fragment(
        fragment: Self::OptionalFragment,
        validator: Validator,
    ) -> Result<Self, ValidationError>;
}
impl<T: Atomic> FromFragment for T {
    type RequiredFragment = T;
    type OptionalFragment = Option<T>;

    fn from_fragment(
        fragment: Self::OptionalFragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        fragment.ok_or_else(|| validator.error_required())
    }
}
impl<T: FromFragment> FromFragment for Option<T> {
    type RequiredFragment = Option<T::RequiredFragment>;
    type OptionalFragment = Option<T::RequiredFragment>;

    fn from_fragment(
        fragment: Self::OptionalFragment,
        validator: Validator,
    ) -> Result<Self, ValidationError> {
        if let Some(fragment) = fragment {
            T::from_fragment(fragment.into(), validator).map(Some)
        } else {
            Ok(None)
        }
    }
}

pub fn validate<T: FromFragment>(fragment: T::OptionalFragment) -> Result<T, ValidationError> {
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
    use super::{validate, Fragment};

    #[derive(Fragment, Debug, PartialEq, Eq)]
    #[fragment(path_overrides(fragment = "super"))]
    #[fragment_attrs(derive(Debug))]
    struct Empty {}

    #[derive(Fragment, Debug, PartialEq, Eq)]
    #[fragment(path_overrides(fragment = "super"))]
    #[fragment_attrs(derive(Default))]
    struct WithFields {
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
}
