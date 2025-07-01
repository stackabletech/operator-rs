/*
   Keep this around for now, it could be useful, because it allows performing Kubernetes checks
   before actually sending a request and waiting for it to fail.

   Warning: You should be sure that Kubernetes enforces these rules for the request you are trying
   to validate.
*/

// This is adapted from Kubernetes.
// See apimachinery/pkg/util/validation/validation.go, apimachinery/pkg/api/validation/generic.go and pkg/apis/core/validation/validation.go in the Kubernetes source

use std::{fmt::Display, sync::LazyLock};

use const_format::concatcp;
use regex::Regex;
use snafu::Snafu;

/// Minimal length required by RFC 1123 is 63. Up to 255 allowed, unsupported by k8s.
const RFC_1123_LABEL_MAX_LENGTH: usize = 63;
pub const RFC_1123_LABEL_FMT: &str = "[a-zA-Z0-9]([-a-zA-Z0-9]*[a-zA-Z0-9])?";
const RFC_1123_LABEL_ERROR_MSG: &str = "a RFC 1123 label must consist of alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";

/// This is a subdomain's max length in DNS (RFC 1123)
const RFC_1123_SUBDOMAIN_MAX_LENGTH: usize = 253;
const RFC_1123_SUBDOMAIN_FMT: &str =
    concatcp!(RFC_1123_LABEL_FMT, "(\\.", RFC_1123_LABEL_FMT, ")*");

const DOMAIN_MAX_LENGTH: usize = RFC_1123_SUBDOMAIN_MAX_LENGTH;
/// Same as [`RFC_1123_SUBDOMAIN_FMT`], but allows a trailing dot
const DOMAIN_FMT: &str = concatcp!(RFC_1123_SUBDOMAIN_FMT, "\\.?");
const DOMAIN_ERROR_MSG: &str = "a domain must consist of alphanumeric characters, '-' or '.', and must start with an alphanumeric character and end with an alphanumeric character or '.'";

// FIXME: According to https://www.rfc-editor.org/rfc/rfc1035#section-2.3.1 domain names must start with a letter
// (and not a number).
const RFC_1035_LABEL_FMT: &str = "[a-z]([-a-z0-9]*[a-z0-9])?";
const RFC_1035_LABEL_ERROR_MSG: &str = "a DNS-1035 label must consist of lower case alphanumeric characters or '-', start with an alphabetic character, and end with an alphanumeric character";

// This is a label's max length in DNS (RFC 1035)
const RFC_1035_LABEL_MAX_LENGTH: usize = 63;

// Technically Kerberos allows more realm names
// (https://web.mit.edu/kerberos/krb5-1.21/doc/admin/realm_config.html#realm-name),
// however, these are embedded in a lot of configuration files and other strings,
// and will not always be quoted properly.
//
// Hence, restrict them to a reasonable subset. The convention is to use upper-case
// DNS hostnames, so allow all characters used there.
const KERBEROS_REALM_NAME_FMT: &str = "[-.a-zA-Z0-9]+";
const KERBEROS_REALM_NAME_ERROR_MSG: &str =
    "Kerberos realm name must only contain alphanumeric characters, '-', and '.'";

// Lazily initialized regular expressions
pub(crate) static DOMAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{DOMAIN_FMT}$")).expect("failed to compile domain regex")
});

static RFC_1123_LABEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{RFC_1123_LABEL_FMT}$")).expect("failed to compile RFC 1123 label regex")
});

static RFC_1035_LABEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{RFC_1035_LABEL_FMT}$")).expect("failed to compile RFC 1035 label regex")
});

pub(crate) static KERBEROS_REALM_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{KERBEROS_REALM_NAME_FMT}$"))
        .expect("failed to compile Kerberos realm name regex")
});

type Result<T = (), E = Errors> = std::result::Result<T, E>;

/// A collection of errors discovered during validation.
#[derive(Debug)]
pub struct Errors(Vec<Error>);

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, error) in self.0.iter().enumerate() {
            let prefix = match i {
                0 => "",
                _ => ", ",
            };
            write!(f, "{prefix}{error}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Errors {}

/// A single validation error.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(transparent)]
    Regex { source: RegexError },

    #[snafu(display("input is {length} bytes long but must be no more than {max_length}"))]
    TooLong { length: usize, max_length: usize },
}

#[derive(Debug)]
pub struct RegexError {
    /// The primary error message.
    msg: &'static str,

    /// The regex that the input must match.
    regex: &'static str,

    /// Examples of valid inputs (if non-empty).
    examples: &'static [&'static str],
}

impl Display for RegexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            msg,
            regex,
            examples,
        } = self;
        write!(f, "{msg} (")?;
        if !examples.is_empty() {
            for (i, example) in examples.iter().enumerate() {
                let prefix = match i {
                    0 => "e.g.",
                    _ => "or",
                };
                write!(f, "{prefix} {example:?}, ")?;
            }
        }
        write!(f, "regex used for validation is {regex:?})")
    }
}

impl std::error::Error for RegexError {}

/// Returns [`Ok`] if `value`'s length fits within `max_length`.
fn validate_str_length(value: &str, max_length: usize) -> Result<(), Error> {
    if value.len() > max_length {
        TooLongSnafu {
            length: value.len(),
            max_length,
        }
        .fail()
    } else {
        Ok(())
    }
}

/// Returns [`Ok`] if `value` matches `regex`.
fn validate_str_regex(
    value: &str,
    regex: &'static Regex,
    error_msg: &'static str,
    examples: &'static [&'static str],
) -> Result<(), Error> {
    if regex.is_match(value) {
        Ok(())
    } else {
        Err(RegexError {
            msg: error_msg,
            regex: regex
                .as_str()
                // Clean up start/end-of-line markers
                .trim_start_matches('^')
                .trim_end_matches('$'),
            examples,
        }
        .into())
    }
}

/// Returns [`Ok`] if *all* validations are [`Ok`], otherwise returns all errors.
fn validate_all(validations: impl IntoIterator<Item = Result<(), Error>>) -> Result {
    let errors = validations
        .into_iter()
        .filter_map(|res| res.err())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Errors(errors))
    }
}

pub fn is_domain(value: &str) -> Result {
    validate_all([
        validate_str_length(value, DOMAIN_MAX_LENGTH),
        validate_str_regex(
            value,
            &DOMAIN_REGEX,
            DOMAIN_ERROR_MSG,
            &[
                "example.com",
                "example.com.",
                "cluster.local",
                "cluster.local.",
            ],
        ),
    ])
}

/// Tests for a string that conforms to the definition of a label in DNS (RFC 1123).
/// Maximum label length supported by k8s is 63 characters (minimum required).
pub fn is_rfc_1123_label(value: &str) -> Result {
    validate_all([
        validate_str_length(value, RFC_1123_LABEL_MAX_LENGTH),
        validate_str_regex(
            value,
            &RFC_1123_LABEL_REGEX,
            RFC_1123_LABEL_ERROR_MSG,
            &["example-label", "1-label-1"],
        ),
    ])
}

/// Tests for a string that conforms to the definition of a label in DNS (RFC 1035).
pub fn is_rfc_1035_label(value: &str) -> Result {
    validate_all([
        validate_str_length(value, RFC_1035_LABEL_MAX_LENGTH),
        validate_str_regex(
            value,
            &RFC_1035_LABEL_REGEX,
            RFC_1035_LABEL_ERROR_MSG,
            &["my-name", "abc-123"],
        ),
    ])
}

/// Tests whether a string looks like a reasonable Kerberos realm name.
///
/// This check is much stricter than krb5's own validation,
pub fn is_kerberos_realm_name(value: &str) -> Result {
    validate_all([validate_str_regex(
        value,
        &KERBEROS_REALM_NAME_REGEX,
        KERBEROS_REALM_NAME_ERROR_MSG,
        &["EXAMPLE.COM"],
    )])
}

// mask_trailing_dash replaces the final character of a string with a subdomain safe
// value if is a dash.
fn mask_trailing_dash(mut name: String) -> String {
    if name.ends_with('-') {
        name.pop();
        name.push('a');
    }

    name
}

/// name_is_dns_label checks whether the passed in name is a valid DNS label
/// according to RFC 1035.
///
/// # Arguments
///
/// * `name` - is the name to check for validity
/// * `prefix` - indicates whether `name` is just a prefix (ending in a dash, which would otherwise not be legal at the end)
pub fn name_is_dns_label(name: &str, prefix: bool) -> Result {
    let mut name = name.to_string();
    if prefix {
        name = mask_trailing_dash(name);
    }

    is_rfc_1035_label(&name)
}

/// Validates a namespace name.
///
/// See [`name_is_dns_label`] for more information.
pub fn validate_namespace_name(name: &str, prefix: bool) -> Result {
    name_is_dns_label(name, prefix)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const RFC_1123_SUBDOMAIN_ERROR_MSG: &str = "a RFC 1123 subdomain must consist of alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";

    static RFC_1123_SUBDOMAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!("^{RFC_1123_SUBDOMAIN_FMT}$"))
            .expect("failed to compile RFC 1123 subdomain regex")
    });

    /// Tests for a string that conforms to the definition of a subdomain in DNS (RFC 1123).
    fn is_rfc_1123_subdomain(value: &str) -> Result {
        validate_all([
            validate_str_length(value, RFC_1123_SUBDOMAIN_MAX_LENGTH),
            validate_str_regex(
                value,
                &RFC_1123_SUBDOMAIN_REGEX,
                RFC_1123_SUBDOMAIN_ERROR_MSG,
                &["example.com"],
            ),
        ])
    }

    #[rstest]
    #[case("")]
    #[case("-")]
    #[case("a-")]
    #[case("-a")]
    #[case("1-")]
    #[case("-1")]
    #[case("_")]
    #[case("a_")]
    #[case("_a")]
    #[case("a_b")]
    #[case("1_")]
    #[case("_1")]
    #[case("1_2")]
    #[case(".")]
    #[case("a.")]
    #[case(".a")]
    #[case("a..b")]
    #[case("1.")]
    #[case(".1")]
    #[case("1..2")]
    #[case(" ")]
    #[case("a ")]
    #[case(" a")]
    #[case("a b")]
    #[case("1 ")]
    #[case(" 1")]
    #[case("1 2")]
    #[case("a@b")]
    #[case("a,b")]
    #[case("a_b")]
    #[case("a;b")]
    #[case("a:b")]
    #[case("a%b")]
    #[case("a?b")]
    #[case("a$b")]
    #[case(&"a".repeat(254))]
    fn is_rfc_1123_subdomain_fail(#[case] value: &str) {
        assert!(is_rfc_1123_subdomain(value).is_err());
    }

    #[rstest]
    #[case("a")]
    #[case("A")]
    #[case("ab")]
    #[case("abc")]
    #[case("aBc")]
    #[case("ABC")]
    #[case("a1")]
    #[case("A1")]
    #[case("a-1")]
    #[case("A-1")]
    #[case("a--1--2--b")]
    #[case("0")]
    #[case("01")]
    #[case("012")]
    #[case("1a")]
    #[case("1-a")]
    #[case("1-A")]
    #[case("1--a--b--2")]
    #[case("a.a")]
    #[case("A.a")]
    #[case("ab.a")]
    #[case("aB.a")]
    #[case("ab.A")]
    #[case("abc.a")]
    #[case("a1.a")]
    #[case("A1.a")]
    #[case("a1.A")]
    #[case("a-1.a")]
    #[case("a--1--2--b.a")]
    #[case("a.1")]
    #[case("A.1")]
    #[case("ab.1")]
    #[case("aB.1")]
    #[case("abc.1")]
    #[case("a1.1")]
    #[case("A1.1")]
    #[case("a-1.1")]
    #[case("a--1--2--b.1")]
    #[case("0.a")]
    #[case("0.A")]
    #[case("01.a")]
    #[case("01.A")]
    #[case("012.a")]
    #[case("012.A")]
    #[case("1a.a")]
    #[case("1A.a")]
    #[case("1a.A")]
    #[case("1-a.a")]
    #[case("1--a--b--2")]
    #[case("0.1")]
    #[case("01.1")]
    #[case("012.1")]
    #[case("1a.1")]
    #[case("1A.1")]
    #[case("1-a.1")]
    #[case("1--a--b--2.1")]
    #[case("a.b.c.d.e")]
    #[case("a.B.c.d.e")]
    #[case("A.B.C.D.E")]
    #[case("aa.bb.cc.dd.ee")]
    #[case("aa.bB.cc.dd.ee")]
    #[case("AA.BB.CC.DD.EE")]
    #[case("1.2.3.4.5")]
    #[case("11.22.33.44.55")]
    #[case(&"a".repeat(253))]
    fn is_rfc_1123_subdomain_pass(#[case] value: &str) {
        assert!(is_rfc_1123_subdomain(value).is_ok());
        // Every valid RFC1123 is also a valid domain
        assert!(is_domain(value).is_ok());
    }

    #[rstest]
    #[case("cluster.local")]
    #[case("CLUSTER.LOCAL")]
    #[case("cluster.local.")]
    #[case("CLUSTER.LOCAL.")]
    fn is_domain_pass(#[case] value: &str) {
        assert!(is_domain(value).is_ok());
    }

    #[test]
    fn test_mask_trailing_dash() {
        assert_eq!(mask_trailing_dash("abc-".to_string()), "abca");
        assert_eq!(mask_trailing_dash("abc".to_string()), "abc");
        assert_eq!(mask_trailing_dash(String::new()), String::new());
        assert_eq!(mask_trailing_dash("-".to_string()), "a");
    }

    #[rstest]
    #[case("0")]
    #[case("01")]
    #[case("012")]
    #[case("1a")]
    #[case("1-a")]
    #[case("1--a--b--2")]
    #[case("")]
    #[case("A")]
    #[case("ABC")]
    #[case("aBc")]
    #[case("A1")]
    #[case("A-1")]
    #[case("1-A")]
    #[case("-")]
    #[case("a-")]
    #[case("-a")]
    #[case("1-")]
    #[case("-1")]
    #[case("_")]
    #[case("a_")]
    #[case("_a")]
    #[case("a_b")]
    #[case("1_")]
    #[case("_1")]
    #[case("1_2")]
    #[case(".")]
    #[case("a.")]
    #[case(".a")]
    #[case("a.b")]
    #[case("1.")]
    #[case(".1")]
    #[case("1.2")]
    #[case(" ")]
    #[case("a ")]
    #[case(" a")]
    #[case("a b")]
    #[case("1 ")]
    #[case(" 1")]
    #[case("1 2")]
    #[case(&"a".repeat(64))]
    fn is_rfc_1035_label_fail(#[case] value: &str) {
        assert!(is_rfc_1035_label(value).is_err());
    }

    #[rstest]
    #[case("a")]
    #[case("ab")]
    #[case("abc")]
    #[case("a1")]
    #[case("a-1")]
    #[case("a--1--2--b")]
    #[case(&"a".repeat(63))]
    fn is_rfc_1035_label_pass(#[case] value: &str) {
        assert!(is_rfc_1035_label(value).is_ok());
    }
}
