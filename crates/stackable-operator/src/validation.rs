/*
   Keep this around for now, it could be useful, because it allows performing Kubernetes checks
   before actually sending a request and waiting for it to fail.

   Warning: You should be sure that Kubernetes enforces these rules for the request you are trying
   to validate.
*/

// This is adapted from Kubernetes.
// See apimachinery/pkg/util/validation/validation.go, apimachinery/pkg/api/validation/generic.go and pkg/apis/core/validation/validation.go in the Kubernetes source

use std::sync::LazyLock;

use const_format::concatcp;
use regex::Regex;

const RFC_1123_LABEL_FMT: &str = "[a-z0-9]([-a-z0-9]*[a-z0-9])?";
const RFC_1123_SUBDOMAIN_FMT: &str =
    concatcp!(RFC_1123_LABEL_FMT, "(\\.", RFC_1123_LABEL_FMT, ")*");
const RFC_1123_SUBDOMAIN_ERROR_MSG: &str = "a lowercase RFC 1123 subdomain must consist of lower case alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";
const RFC_1123_LABEL_ERROR_MSG: &str = "a lowercase RFC 1123 label must consist of lower case alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";

// This is a subdomain's max length in DNS (RFC 1123)
const RFC_1123_SUBDOMAIN_MAX_LENGTH: usize = 253;
// Minimal length reuquired by RFC 1123 is 63. Up to 255 allowed, unsupported by k8s.
const RFC_1123_LABEL_MAX_LENGTH: usize = 63;

const RFC_1035_LABEL_FMT: &str = "[a-z]([-a-z0-9]*[a-z0-9])?";
const RFC_1035_LABEL_ERR_MSG: &str = "a DNS-1035 label must consist of lower case alphanumeric characters or '-', start with an alphabetic character, and end with an alphanumeric character";

// This is a label's max length in DNS (RFC 1035)
const RFC_1035_LABEL_MAX_LENGTH: usize = 63;

// Lazily initialized regular expressions
static RFC_1123_SUBDOMAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{RFC_1123_SUBDOMAIN_FMT}$"))
        .expect("failed to compile RFC 1123 subdomain regex")
});

static RFC_1035_LABEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^{RFC_1035_LABEL_FMT}$")).expect("failed to compile RFC 1035 label regex")
});

/// Returns a formatted error message for maximum length violations.
fn max_len_error(length: usize) -> String {
    format!("must be no more than {length} characters")
}

/// Returns a formatted error message for regex violations.
///
/// # Arguments
///
/// * `msg` - this is the main error message to return
/// * `fmt` - this is the regular expression that did not match the input
/// * `examples` - are optional well, formed examples that would match the regex
fn regex_error(msg: &str, fmt: &str, examples: &[&str]) -> String {
    if examples.is_empty() {
        return format!("{msg} (regex used for validation is '{fmt}')");
    }

    let mut msg = msg.to_string();
    msg.push_str(" (e.g. ");
    for (i, example) in examples.iter().enumerate() {
        if i > 0 {
            msg.push_str(" or ");
        }
        msg.push('\'');
        msg.push_str(example);
        msg.push_str("', ");
    }

    msg.push_str("regex used for validation is '");
    msg.push_str(fmt);
    msg.push_str("')");
    msg
}

/// Tests for a string that conforms to the definition of a subdomain in DNS (RFC 1123).
pub fn is_rfc_1123_subdomain(value: &str) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    if value.len() > RFC_1123_SUBDOMAIN_MAX_LENGTH {
        errors.push(max_len_error(RFC_1123_SUBDOMAIN_MAX_LENGTH))
    }

    if !RFC_1123_SUBDOMAIN_REGEX.is_match(value) {
        errors.push(regex_error(
            RFC_1123_SUBDOMAIN_ERROR_MSG,
            RFC_1123_SUBDOMAIN_FMT,
            &["example.com"],
        ))
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Tests for a string that conforms to the definition of a label in DNS (RFC 1123).
/// Maximum label length supported by k8s is 63 characters (minimum required).
pub fn is_rfc_1123_label(value: &str) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    if value.len() > RFC_1123_LABEL_MAX_LENGTH {
        errors.push(max_len_error(RFC_1123_LABEL_MAX_LENGTH))
    }

    // Regex is identical to RFC 1123 subdomain
    if !RFC_1123_SUBDOMAIN_REGEX.is_match(value) {
        errors.push(regex_error(
            RFC_1123_LABEL_ERROR_MSG,
            RFC_1123_SUBDOMAIN_FMT,
            &["example-label", "1-label-1"],
        ))
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Tests for a string that conforms to the definition of a label in DNS (RFC 1035).
pub fn is_rfc_1035_label(value: &str) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    if value.len() > RFC_1035_LABEL_MAX_LENGTH {
        errors.push(max_len_error(RFC_1035_LABEL_MAX_LENGTH))
    }

    if !RFC_1035_LABEL_REGEX.is_match(value) {
        errors.push(regex_error(
            RFC_1035_LABEL_ERR_MSG,
            RFC_1035_LABEL_FMT,
            &["my-name", "abc-123"],
        ))
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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

/// name_is_dns_subdomain checks whether the passed in name is a valid
/// DNS subdomain name
///
/// # Arguments
///
/// * `name` - is the name to check for validity
/// * `prefix` - indicates whether `name` is just a prefix (ending in a dash, which would otherwise not be legal at the end)
pub fn name_is_dns_subdomain(name: &str, prefix: bool) -> Result<(), Vec<String>> {
    let mut name = name.to_string();
    if prefix {
        name = mask_trailing_dash(name);
    }

    is_rfc_1123_subdomain(&name)
}

/// name_is_dns_label checks whether the passed in name is a valid DNS label
/// according to RFC 1035.
///
/// # Arguments
///
/// * `name` - is the name to check for validity
/// * `prefix` - indicates whether `name` is just a prefix (ending in a dash, which would otherwise not be legal at the end)
pub fn name_is_dns_label(name: &str, prefix: bool) -> Result<(), Vec<String>> {
    let mut name = name.to_string();
    if prefix {
        name = mask_trailing_dash(name);
    }

    is_rfc_1035_label(&name)
}

/// Validates a namespace name.
///
/// See [`name_is_dns_label`] for more information.
pub fn validate_namespace_name(name: &str, prefix: bool) -> Result<(), Vec<String>> {
    name_is_dns_label(name, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
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
    #[case("A.a")]
    #[case("aB.a")]
    #[case("ab.A")]
    #[case("A1.a")]
    #[case("a1.A")]
    #[case("A.1")]
    #[case("aB.1")]
    #[case("A1.1")]
    #[case("1A.1")]
    #[case("0.A")]
    #[case("01.A")]
    #[case("012.A")]
    #[case("1A.a")]
    #[case("1a.A")]
    #[case("A.B.C.D.E")]
    #[case("AA.BB.CC.DD.EE")]
    #[case("a.B.c.d.e")]
    #[case("aa.bB.cc.dd.ee")]
    #[case("a@b")]
    #[case("a,b")]
    #[case("a_b")]
    #[case("a;b")]
    #[case("a:b")]
    #[case("a%b")]
    #[case("a?b")]
    #[case("a$b")]
    #[case(&"a".repeat(254))]
    fn test_bad_values_is_rfc_1123_subdomain(#[case] value: &str) {
        assert!(is_rfc_1123_subdomain(value).is_err());
    }

    #[rstest]
    #[case("a")]
    #[case("ab")]
    #[case("abc")]
    #[case("a1")]
    #[case("a-1")]
    #[case("a--1--2--b")]
    #[case("0")]
    #[case("01")]
    #[case("012")]
    #[case("1a")]
    #[case("1-a")]
    #[case("1--a--b--2")]
    #[case("a.a")]
    #[case("ab.a")]
    #[case("abc.a")]
    #[case("a1.a")]
    #[case("a-1.a")]
    #[case("a--1--2--b.a")]
    #[case("a.1")]
    #[case("ab.1")]
    #[case("abc.1")]
    #[case("a1.1")]
    #[case("a-1.1")]
    #[case("a--1--2--b.1")]
    #[case("0.a")]
    #[case("01.a")]
    #[case("012.a")]
    #[case("1a.a")]
    #[case("1-a.a")]
    #[case("1--a--b--2")]
    #[case("0.1")]
    #[case("01.1")]
    #[case("012.1")]
    #[case("1a.1")]
    #[case("1-a.1")]
    #[case("1--a--b--2.1")]
    #[case("a.b.c.d.e")]
    #[case("aa.bb.cc.dd.ee")]
    #[case("1.2.3.4.5")]
    #[case("11.22.33.44.55")]
    #[case(&"a".repeat(253))]
    fn test_good_values_is_rfc_1123_subdomain(#[case] value: &str) {
        assert!(is_rfc_1123_subdomain(value).is_ok());
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
    fn test_bad_values_is_rfc_1035_label(#[case] value: &str) {
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
    fn test_good_values_is_rfc_1035_label(#[case] value: &str) {
        assert!(is_rfc_1035_label(value).is_ok());
    }
}
