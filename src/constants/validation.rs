use const_format::concatcp;

pub(crate) const RFC_1123_LABEL_FMT: &str = "[a-z0-9]([-a-z0-9]*[a-z0-9])?";
pub(crate) const RFC_1123_SUBDOMAIN_FMT: &str =
    concatcp!(RFC_1123_LABEL_FMT, "(\\.", RFC_1123_LABEL_FMT, ")*");
pub(crate) const RFC_1123_SUBDOMAIN_ERROR_MSG: &str = "a lowercase RFC 1123 subdomain must consist of lower case alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";
pub(crate) const RFC_1123_LABEL_ERROR_MSG: &str = "a lowercase RFC 1123 label must consist of lower case alphanumeric characters, '-' or '.', and must start and end with an alphanumeric character";

// This is a subdomain's max length in DNS (RFC 1123)
pub const RFC_1123_SUBDOMAIN_MAX_LENGTH: usize = 253;
// Minimal length reuquired by RFC 1123 is 63. Up to 255 allowed, unsupported by k8s.
pub const RFC_1123_LABEL_MAX_LENGTH: usize = 63;

pub(crate) const RFC_1035_LABEL_FMT: &str = "[a-z]([-a-z0-9]*[a-z0-9])?";
pub(crate) const RFC_1035_LABEL_ERR_MSG: &str = "a DNS-1035 label must consist of lower case alphanumeric characters or '-', start with an alphabetic character, and end with an alphanumeric character";

// This is a label's max length in DNS (RFC 1035)
pub const RFC_1035_LABEL_MAX_LENGTH: usize = RFC_1123_LABEL_MAX_LENGTH;
