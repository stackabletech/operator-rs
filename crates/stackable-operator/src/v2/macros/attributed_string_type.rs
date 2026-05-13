use snafu::Snafu;
use strum::{EnumDiscriminants, IntoStaticStr};

/// Maximum length of label values
///
/// Duplicates the private constant [`stackable_operator::kvp::LABEL_VALUE_MAX_LEN`]
pub const MAX_LABEL_VALUE_LENGTH: usize = 63;

#[derive(Debug, EnumDiscriminants, Snafu)]
#[snafu(visibility(pub))]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("minimum length not met"))]
    MinimumLengthNotMet { length: usize, min_length: usize },

    #[snafu(display("maximum length exceeded"))]
    LengthExceeded { length: usize, max_length: usize },

    #[snafu(display("invalid regular expression"))]
    InvalidRegex { source: regex::Error },

    #[snafu(display("regular expression not matched"))]
    RegexNotMatched { value: String, regex: &'static str },

    #[snafu(display("not a valid label value"))]
    InvalidLabelValue {
        source: stackable_operator::kvp::LabelValueError,
    },

    #[snafu(display("not a valid label name as defined in RFC 1035"))]
    InvalidRfc1035LabelName {
        source: stackable_operator::validation::Errors,
    },

    #[snafu(display("not a valid DNS subdomain name as defined in RFC 1123"))]
    InvalidRfc1123DnsSubdomainName {
        source: stackable_operator::validation::Errors,
    },

    #[snafu(display("not a valid label name as defined in RFC 1123"))]
    InvalidRfc1123LabelName {
        source: stackable_operator::validation::Errors,
    },

    #[snafu(display("not a valid UUID"))]
    InvalidUid { source: uuid::Error },
}

/// Helper data type to determine combined regular expressions
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Regex {
    /// There is a regular expression but it is unknown (because it was too complicated to
    /// calculate it).
    Unknown,

    /// `MatchAll` equals `Expression(".*")`, but `MatchAll` can be pattern matched in a const
    /// context, whereas `Expression(...)` cannot.
    MatchAll,

    /// A regular expression
    Expression(&'static str),
}

impl Regex {
    /// Combine this regular expression with the given one.
    pub const fn combine(self, other: Regex) -> Regex {
        match (self, other) {
            (_, Regex::MatchAll) => self,
            (Regex::MatchAll, _) => other,
            // It is hard to combine two regular expressions and nearly impossible to do this in a
            // const context. Fortunately, for most of the data types, only one regular expression
            // is set.
            _ => Regex::Unknown,
        }
    }
}

/// Restricted string type with attributes like maximum length.
///
/// Fully-qualified types are used to ease the import into other modules.
///
/// # Examples
///
/// ```rust
/// attributed_string_type! {
///     ConfigMapName,
///     "The name of a ConfigMap",
///     "opensearch-nodes-default",
///     is_rfc_1123_dns_subdomain_name
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! attributed_string_type {
    ($name:ident, $description:literal, $example:literal $(, $attribute:tt)*) => {
        #[doc = std::concat!($description, ", e.g. \"", $example, "\"")]
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// The minimum length
            pub const MIN_LENGTH: usize = attributed_string_type!(@min_length $($attribute)*);

            /// The maximum length
            pub const MAX_LENGTH: usize = attributed_string_type!(@max_length $($attribute)*);

            /// The regular expression
            ///
            /// This field is not meant to be used outside of this macro.
            pub const REGEX: $crate::framework::macros::attributed_string_type::Regex = attributed_string_type!(@regex $($attribute)*);
        }

        impl stackable_operator::config::merge::Atomic for $name {}

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<&$name> for String {
            fn from(value: &$name) -> Self {
                value.0.clone()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::framework::macros::attributed_string_type::Error;

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                // ResultExt::context is used on most but not all usages of this macro
                #[allow(unused_imports)]
                use snafu::ResultExt;

                $(attributed_string_type!(@from_str $name, s, $attribute);)*

                Ok(Self(s.to_owned()))
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let string: String = serde::Deserialize::deserialize(deserializer)?;
                $name::from_str(&string).map_err(|err| serde::de::Error::custom(&err))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        // The JsonSchema implementation requires `max_length`.
        impl stackable_operator::schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                std::stringify!($name).into()
            }

            fn json_schema(_generator: &mut stackable_operator::schemars::generate::SchemaGenerator) -> stackable_operator::schemars::Schema {
                stackable_operator::schemars::json_schema!({
                    "type": "string",
                    "minLength": $name::MIN_LENGTH,
                    "maxLength": if $name::MAX_LENGTH != usize::MAX {
                        Some($name::MAX_LENGTH)
                    } else {
                        // Do not set maxLength if it is usize::MAX.
                        None
                    },
                    "pattern": match $name::REGEX {
                        $crate::framework::macros::attributed_string_type::Regex::Expression(regex) => Some(regex),
                        _ => None
                    }
                })
            }
        }

        #[cfg(test)]
        impl $name {
            #[allow(dead_code)]
            pub fn from_str_unsafe(s: &str) -> Self {
                std::str::FromStr::from_str(s).expect("should be a valid {name}")
            }

            // A dead_code warning is emitted if there is no unit test that calls this function.
            pub fn test_example() {
                Self::from_str_unsafe($example);
            }
        }

        $(attributed_string_type!(@trait_impl $name, $attribute);)*
    };

    // std::str::FromStr

    (@from_str $name:ident, $s:expr, (min_length = $min_length:expr)) => {
        let length = $s.len() as usize;
        snafu::ensure!(
            length >= $name::MIN_LENGTH,
            $crate::framework::macros::attributed_string_type::MinimumLengthNotMetSnafu {
                length,
                min_length: $name::MIN_LENGTH,
            }
        );
    };
    (@from_str $name:ident, $s:expr, (max_length = $max_length:expr)) => {
        let length = $s.len() as usize;
        snafu::ensure!(
            length <= $name::MAX_LENGTH,
            $crate::framework::macros::attributed_string_type::LengthExceededSnafu {
                length,
                max_length: $name::MAX_LENGTH,
            }
        );
    };
    (@from_str $name:ident, $s:expr, (regex = $regex:expr)) => {
        let regex = regex::Regex::new($regex).context($crate::framework::macros::attributed_string_type::InvalidRegexSnafu)?;
        snafu::ensure!(
            regex.is_match($s),
            $crate::framework::macros::attributed_string_type::RegexNotMatchedSnafu {
                value: $s,
                regex: $regex
            }
        );
    };
    (@from_str $name:ident, $s:expr, is_rfc_1035_label_name) => {
        stackable_operator::validation::is_lowercase_rfc_1035_label($s).context($crate::framework::macros::attributed_string_type::InvalidRfc1035LabelNameSnafu)?;
    };
    (@from_str $name:ident, $s:expr, is_rfc_1123_dns_subdomain_name) => {
        stackable_operator::validation::is_lowercase_rfc_1123_subdomain($s).context($crate::framework::macros::attributed_string_type::InvalidRfc1123DnsSubdomainNameSnafu)?;
    };
    (@from_str $name:ident, $s:expr, is_rfc_1123_label_name) => {
        stackable_operator::validation::is_lowercase_rfc_1123_label($s).context($crate::framework::macros::attributed_string_type::InvalidRfc1123LabelNameSnafu)?;
    };
    (@from_str $name:ident, $s:expr, is_valid_label_value) => {
        stackable_operator::kvp::LabelValue::from_str($s).context($crate::framework::macros::attributed_string_type::InvalidLabelValueSnafu)?;
    };
    (@from_str $name:ident, $s:expr, is_uid) => {
        uuid::Uuid::try_parse($s).context($crate::framework::macros::attributed_string_type::InvalidUidSnafu)?;
    };

    // MIN_LENGTH

    (@min_length) => {
        // The minimum String length is 0.
        0
    };
    (@min_length (min_length = $min_length:expr) $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            $min_length,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };
    (@min_length (max_length = $max_length:expr) $($attribute:tt)*) => {
        // max_length has no opinion on the min_length.
        attributed_string_type!(@min_length $($attribute)*)
    };
    (@min_length (regex = $regex:expr) $($attribute:tt)*) => {
        // regex has no influence on the min_length.
        attributed_string_type!(@min_length $($attribute)*)
    };
    (@min_length is_rfc_1035_label_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            1,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };
    (@min_length is_rfc_1123_dns_subdomain_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            1,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };
    (@min_length is_rfc_1123_label_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            1,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };
    (@min_length is_valid_label_value $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            1,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };
    (@min_length is_uid $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::max(
            uuid::fmt::Hyphenated::LENGTH,
            attributed_string_type!(@min_length $($attribute)*)
        )
    };

    // MAX_LENGTH

    (@max_length) => {
        // If there is no other max_length defined, then the upper bound is usize::MAX.
        usize::MAX
    };
    (@max_length (min_length = $min_length:expr) $($attribute:tt)*) => {
        // min_length has no opinion on the max_length.
        attributed_string_type!(@max_length $($attribute)*)
    };
    (@max_length (max_length = $max_length:expr) $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            $max_length,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };
    (@max_length (regex = $regex:expr) $($attribute:tt)*) => {
        // regex has no influence on the max_length.
        attributed_string_type!(@max_length $($attribute)*)
    };
    (@max_length is_rfc_1035_label_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            stackable_operator::validation::RFC_1035_LABEL_MAX_LENGTH,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };
    (@max_length is_rfc_1123_dns_subdomain_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            stackable_operator::validation::RFC_1123_SUBDOMAIN_MAX_LENGTH,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };
    (@max_length is_rfc_1123_label_name $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            stackable_operator::validation::RFC_1123_LABEL_MAX_LENGTH,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };
    (@max_length is_valid_label_value $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            $crate::framework::macros::attributed_string_type::MAX_LABEL_VALUE_LENGTH,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };
    (@max_length is_uid $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::min(
            uuid::fmt::Hyphenated::LENGTH,
            attributed_string_type!(@max_length $($attribute)*)
        )
    };

    // REGEX

    (@regex) => {
        // Everything is allowed if there is no other regular expression.
        $crate::framework::macros::attributed_string_type::Regex::MatchAll
    };
    (@regex (min_length = $min_length:expr) $($attribute:tt)*) => {
        // min_length has no influence on the regular expression.
        attributed_string_type!(@regex $($attribute)*)
    };
    (@regex (max_length = $max_length:expr) $($attribute:tt)*) => {
        // max_length has no influence on the regular expression.
        attributed_string_type!(@regex $($attribute)*)
    };
    (@regex (regex = $regex:expr) $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::Regex::Expression($regex)
            .combine(attributed_string_type!(@regex $($attribute)*))
    };
    (@regex is_rfc_1035_label_name $($attribute:tt)*) => {
        // see https://github.com/kubernetes/kubernetes/blob/v1.35.0/staging/src/k8s.io/apimachinery/pkg/util/validation/validation.go#L228
        $crate::framework::macros::attributed_string_type::Regex::Expression("^[a-z]([-a-z0-9]*[a-z0-9])?$")
            .combine(attributed_string_type!(@regex $($attribute)*))
    };
    (@regex is_rfc_1123_dns_subdomain_name $($attribute:tt)*) => {
        // see https://github.com/kubernetes/kubernetes/blob/v1.35.0/staging/src/k8s.io/apimachinery/pkg/util/validation/validation.go#L193
        $crate::framework::macros::attributed_string_type::Regex::Expression("^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")
            .combine(attributed_string_type!(@regex $($attribute)*))
    };
    (@regex is_rfc_1123_label_name $($attribute:tt)*) => {
        // see https://github.com/kubernetes/kubernetes/blob/v1.35.0/staging/src/k8s.io/apimachinery/pkg/util/validation/validation.go#L163
        $crate::framework::macros::attributed_string_type::Regex::Expression("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")
            .combine(attributed_string_type!(@regex $($attribute)*))
    };
    (@regex is_valid_label_value $($attribute:tt)*) => {
        // regular expression from stackable_operator::kvp::label::LABEL_VALUE_REGEX
        $crate::framework::macros::attributed_string_type::Regex::Expression("^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$")
            .combine(attributed_string_type!(@regex $($attribute)*))
    };
    (@regex is_uid $($attribute:tt)*) => {
        $crate::framework::macros::attributed_string_type::Regex::Expression("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .combine(attributed_string_type!(@regex $($attribute)*))
    };

    // additional constants and trait implementations

    (@trait_impl $name:ident, (min_length = $max_length:expr)) => {
    };
    (@trait_impl $name:ident, (max_length = $max_length:expr)) => {
    };
    (@trait_impl $name:ident, (regex = $regex:expr)) => {
    };
    (@trait_impl $name:ident, is_rfc_1035_label_name) => {
        impl $name {
            pub const IS_RFC_1035_LABEL_NAME: bool = true;
            pub const IS_RFC_1123_LABEL_NAME: bool = true;
            pub const IS_RFC_1123_SUBDOMAIN_NAME: bool = true;
        }
    };
    (@trait_impl $name:ident, is_rfc_1123_dns_subdomain_name) => {
        impl $name {
            pub const IS_RFC_1123_SUBDOMAIN_NAME: bool = true;
        }
    };
    (@trait_impl $name:ident, is_rfc_1123_label_name) => {
        impl $name {
            pub const IS_RFC_1123_LABEL_NAME: bool = true;
            pub const IS_RFC_1123_SUBDOMAIN_NAME: bool = true;
        }
    };
    (@trait_impl $name:ident, is_valid_label_value) => {
        impl $name {
            pub const IS_VALID_LABEL_VALUE: bool = true;
        }

        impl $crate::framework::NameIsValidLabelValue for $name {
            fn to_label_value(&self) -> String {
                self.0.clone()
            }
        }
    };
    (@trait_impl $name:ident, is_uid) => {
        impl From<uuid::Uuid> for $name {
            fn from(value: uuid::Uuid) -> Self {
                Self(value.to_string())
            }
        }

        impl From<&uuid::Uuid> for $name {
            fn from(value: &uuid::Uuid) -> Self {
                Self(value.to_string())
            }
        }
    };
}

/// Returns the minimum of the given values.
///
/// As opposed to [`std::cmp::min`], this function can be used at compile-time.
///
/// # Examples
///
/// ```rust
/// assert_eq!(2, min(2, 3));
/// assert_eq!(4, min(5, 4));
/// assert_eq!(1, min(1, 1));
/// ```
pub const fn min(x: usize, y: usize) -> usize {
    if x < y { x } else { y }
}

/// Returns the maximum of the given values.
///
/// As opposed to [`std::cmp::max`], this function can be used at compile-time.
///
/// # Examples
///
/// ```rust
/// assert_eq!(3, max(2, 3));
/// assert_eq!(5, max(5, 4));
/// assert_eq!(1, max(1, 1));
/// ```
pub const fn max(x: usize, y: usize) -> usize {
    if x < y { y } else { x }
}

#[cfg(test)]
// `InvalidRegexTest` intentionally contains an invalid regular expression.
#[allow(clippy::invalid_regex)]
mod tests {
    use std::str::FromStr;

    use serde_json::{Number, Value, json};
    use stackable_operator::schemars::{JsonSchema, SchemaGenerator};
    use uuid::uuid;

    use super::{ErrorDiscriminants, Regex};
    use crate::framework::NameIsValidLabelValue;

    attributed_string_type! {
        MinLengthWithoutConstraintsTest,
        "min_length test without constraints",
        ""
    }

    #[test]
    fn test_attributed_string_type_min_length_without_constraints() {
        type T = MinLengthWithoutConstraintsTest;

        T::test_example();
        assert_eq!(0, T::MIN_LENGTH);
    }

    attributed_string_type! {
        MinLengthWithConstraintsTest,
        "min_length test with constraints",
        "test",
        (min_length = 2), // should set the minimum length to 2
        (max_length = 8), // should not affect the minimum length
        (regex = "^.{4}$"), // should not affect the minimum length
        is_rfc_1035_label_name, // should be overruled by the greater min_length
        is_valid_label_value // should be overruled by the greater min_length
    }

    #[test]
    fn test_attributed_string_type_min_length_with_constraints() {
        type T = MinLengthWithConstraintsTest;

        T::test_example();
        assert_eq!(2, T::MIN_LENGTH);
        assert_eq!(
            Err(ErrorDiscriminants::MinimumLengthNotMet),
            T::from_str("a").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        MaxLengthWithoutConstraintsTest,
        "max_length test without constraints",
        ""
    }

    #[test]
    fn test_attributed_string_type_max_length_without_constraints() {
        type T = MaxLengthWithoutConstraintsTest;

        T::test_example();
        assert_eq!(usize::MAX, T::MAX_LENGTH);
    }

    attributed_string_type! {
        MaxLengthWithConstraintsTest,
        "max_length test with constraints",
        "test",
        (min_length = 2), // should not affect the maximum length
        (max_length = 8), // should set the maximum length to 8
        (regex = "^.{4}$"), // should not affect the maximum length
        is_rfc_1035_label_name, // should be overruled by the lower max_length
        is_valid_label_value // should be overruled by the lower max_length
    }

    #[test]
    fn test_attributed_string_type_max_length_with_constraints() {
        type T = MaxLengthWithConstraintsTest;

        T::test_example();
        assert_eq!(8, T::MAX_LENGTH);
        assert_eq!(
            Err(ErrorDiscriminants::LengthExceeded),
            T::from_str("test-12345").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        RegexWithoutConstraintsTest,
        "regex test without constraints",
        ""
    }

    #[test]
    fn test_attributed_string_type_regex_without_constraints() {
        type T = RegexWithoutConstraintsTest;

        T::test_example();
        assert_eq!(Regex::MatchAll, T::REGEX);
    }

    attributed_string_type! {
        RegexWithOneConstraintTest,
        "regex test with one constraint",
        "test",
        (min_length = 2), // should not affect the regular expression
        (max_length = 8), // should not affect the regular expression
        (regex = "^[est]{4}$") // should set the regular expression to "[est]{4}"
    }

    #[test]
    fn test_attributed_string_type_regex_with_one_constraint() {
        type T = RegexWithOneConstraintTest;

        T::test_example();
        assert_eq!(Regex::Expression("^[est]{4}$"), T::REGEX);
        assert_eq!(
            Err(ErrorDiscriminants::RegexNotMatched),
            T::from_str("t-st").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        RegexWithMultipleConstraintsTest,
        "regex test with multiple constraints",
        "test",
        (min_length = 2), // should not affect the regular expression
        (max_length = 8), // should not affect the regular expression
        (regex = "^[est]{4}$"), // should not be combinable with is_rfc_1123_dns_subdomain_name
        is_rfc_1123_dns_subdomain_name // should not be combinable with regex
    }

    #[test]
    fn test_attributed_string_type_regex_with_multiple_constraints() {
        type T = RegexWithMultipleConstraintsTest;

        T::test_example();
        assert_eq!(Regex::Unknown, T::REGEX);
        assert_eq!(
            Err(ErrorDiscriminants::RegexNotMatched),
            T::from_str("t-st").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        InvalidRegexTest,
        "regex test with invalid expression",
        "test",
        (min_length = 2), // should not affect the regular expression
        (max_length = 8), // should not affect the regular expression
        (regex = "{") // should throw an error at runtime
    }

    #[test]
    fn test_attributed_string_type_regex_with_invalid_expression() {
        type T = InvalidRegexTest;

        // It is not known yet at compile-time that this expression is invalid.
        assert_eq!(Regex::Expression("{"), T::REGEX);
        assert_eq!(
            Err(ErrorDiscriminants::InvalidRegex),
            T::from_str("test").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        DisplayFmtTest,
        "Display::fmt test",
        "test"
    }

    #[test]
    fn test_attributed_string_type_display_fmt() {
        type T = DisplayFmtTest;

        assert_eq!("test", format!("{}", T::from_str_unsafe("test")));
    }

    attributed_string_type! {
        StringFromTest,
        "String::from test",
        "test"
    }

    #[test]
    fn test_attributed_string_type_string_from() {
        type T = StringFromTest;

        T::test_example();
        assert_eq!("test", String::from(T::from_str_unsafe("test")));
        assert_eq!("test", String::from(&T::from_str_unsafe("test")));
    }

    attributed_string_type! {
        DeserializeTest,
        "serde::Deserialize test",
        "test",
        (min_length = 2),
        (max_length = 4),
        (regex = "^[est-]+$"),
        is_rfc_1035_label_name
    }

    #[test]
    fn test_attributed_string_type_deserialize() {
        type T = DeserializeTest;

        T::test_example();
        assert_eq!(
            T::from_str_unsafe("test"),
            serde_json::from_value(Value::String("test".to_owned()))
                .expect("should be deserializable")
        );
        assert_eq!(
            Err("minimum length not met".to_owned()),
            serde_json::from_value::<T>(Value::String("e".to_owned()))
                .map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("maximum length exceeded".to_owned()),
            serde_json::from_value::<T>(Value::String("testt".to_owned()))
                .map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("regular expression not matched".to_owned()),
            serde_json::from_value::<T>(Value::String("abc".to_owned()))
                .map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("not a valid label name as defined in RFC 1035".to_owned()),
            serde_json::from_value::<T>(Value::String("-tst".to_owned()))
                .map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("invalid type: null, expected a string".to_owned()),
            serde_json::from_value::<T>(Value::Null).map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("invalid type: boolean `true`, expected a string".to_owned()),
            serde_json::from_value::<T>(Value::Bool(true)).map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("invalid type: integer `1`, expected a string".to_owned()),
            serde_json::from_value::<T>(Value::Number(
                Number::from_i128(1).expect("should be a valid number")
            ))
            .map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("invalid type: sequence, expected a string".to_owned()),
            serde_json::from_value::<T>(Value::Array(vec![])).map_err(|err| err.to_string())
        );
        assert_eq!(
            Err("invalid type: map, expected a string".to_owned()),
            serde_json::from_value::<T>(Value::Object(serde_json::Map::new()))
                .map_err(|err| err.to_string())
        );
    }

    attributed_string_type! {
        SerializeTest,
        "serde::Serialize test",
        "test"
    }

    #[test]
    fn test_attributed_string_type_serialize() {
        type T = SerializeTest;

        T::test_example();
        assert_eq!(
            "\"test\"".to_owned(),
            serde_json::to_string(&T::from_str_unsafe("test")).expect("should be serializable")
        );
    }

    attributed_string_type! {
        JsonSchemaWithoutConstraintsTest,
        "JsonSchema test with constraints",
        "test"
    }

    #[test]
    fn test_attributed_string_type_json_schema_without_constaints() {
        type T = JsonSchemaWithoutConstraintsTest;

        T::test_example();
        assert_eq!("JsonSchemaWithoutConstraintsTest", T::schema_name());
        assert_eq!(
            json!({
                "type": "string",
                "minLength": 0,
                "maxLength": None::<usize>,
                "pattern": None::<String>
            }),
            T::json_schema(&mut SchemaGenerator::default())
        );
    }

    attributed_string_type! {
        JsonSchemaWithConstraintsTest,
        "JsonSchema test with constraints",
        "test",
        (min_length = 4),
        (max_length = 8),
        (regex = "^[est]+$")
    }

    #[test]
    fn test_attributed_string_type_json_schema_with_constraints() {
        type T = JsonSchemaWithConstraintsTest;

        T::test_example();
        assert_eq!("JsonSchemaWithConstraintsTest", T::schema_name());
        assert_eq!(
            json!({
                "type": "string",
                "minLength": 4,
                "maxLength": 8,
                "pattern": "^[est]+$"
            }),
            T::json_schema(&mut SchemaGenerator::default())
        );
    }

    attributed_string_type! {
        IsRfc1035LabelNameTest,
        "is_rfc_1035_label_name test",
        "a-b",
        is_rfc_1035_label_name
    }

    #[test]
    fn test_attributed_string_type_is_rfc_1035_label_name() {
        type T = IsRfc1035LabelNameTest;

        let _ = T::IS_RFC_1035_LABEL_NAME;
        let _ = T::IS_RFC_1123_LABEL_NAME;
        let _ = T::IS_RFC_1123_SUBDOMAIN_NAME;

        T::test_example();
        assert_eq!(
            Err(ErrorDiscriminants::InvalidRfc1035LabelName),
            T::from_str("A").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        IsRfc1123DnsSubdomainNameTest,
        "is_rfc_1123_dns_subdomain_name test",
        "a-b.c",
        is_rfc_1123_dns_subdomain_name
    }

    #[test]
    fn test_attributed_string_type_is_rfc_1123_dns_subdomain_name() {
        type T = IsRfc1123DnsSubdomainNameTest;

        let _ = T::IS_RFC_1123_SUBDOMAIN_NAME;

        T::test_example();
        assert_eq!(
            Err(ErrorDiscriminants::InvalidRfc1123DnsSubdomainName),
            T::from_str("A").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        IsRfc1123LabelNameTest,
        "is_rfc_1123_label_name test",
        "1-a",
        is_rfc_1123_label_name
    }

    #[test]
    fn test_attributed_string_type_is_rfc_1123_label_name() {
        type T = IsRfc1123LabelNameTest;

        let _ = T::IS_RFC_1123_LABEL_NAME;
        let _ = T::IS_RFC_1123_SUBDOMAIN_NAME;

        T::test_example();
        assert_eq!(
            Err(ErrorDiscriminants::InvalidRfc1123LabelName),
            T::from_str("A").map_err(ErrorDiscriminants::from)
        );
    }

    attributed_string_type! {
        IsValidLabelValueTest,
        "is_valid_label_value test",
        "a-_.1",
        is_valid_label_value
    }

    #[test]
    fn test_attributed_string_type_is_valid_label_value() {
        type T = IsValidLabelValueTest;

        let _ = T::IS_VALID_LABEL_VALUE;

        T::test_example();
        assert_eq!(
            Err(ErrorDiscriminants::InvalidLabelValue),
            T::from_str("invalid label value").map_err(ErrorDiscriminants::from)
        );
        assert_eq!(
            "label-value",
            T::from_str_unsafe("label-value").to_label_value()
        );
    }

    attributed_string_type! {
        IsUidTest,
        "is_uid test",
        "c27b3971-ca72-42c1-80a4-abdfc1db0ddd",
        is_uid
    }

    #[test]
    fn test_attributed_string_type_is_uid() {
        type T = IsUidTest;

        T::test_example();
        assert_eq!(
            Err(ErrorDiscriminants::InvalidUid),
            T::from_str("invalid UID").map_err(ErrorDiscriminants::from)
        );
        assert_eq!(
            "c27b3971-ca72-42c1-80a4-abdfc1db0ddd",
            T::from(uuid!("c27b3971-ca72-42c1-80a4-abdfc1db0ddd")).to_string()
        );
        assert_eq!(
            "c27b3971-ca72-42c1-80a4-abdfc1db0ddd",
            T::from(&uuid!("c27b3971-ca72-42c1-80a4-abdfc1db0ddd")).to_string()
        );
    }
}
