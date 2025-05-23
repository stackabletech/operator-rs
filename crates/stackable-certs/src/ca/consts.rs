use rsa::pkcs8::LineEnding;
use stackable_operator::time::Duration;

/// The default CA validity time span of one hour.
pub const DEFAULT_CA_VALIDITY: Duration = Duration::from_hours_unchecked(1);

/// The default certificate validity time span of one hour.
pub const DEFAULT_CERTIFICATE_VALIDITY: Duration = Duration::from_hours_unchecked(1);

/// The root CA subject name containing only the common name.
pub const SDP_ROOT_CA_SUBJECT: &str = "CN=Stackable Data Platform Internal CA";

/// As we are mostly on Unix systems, we are using `\ņ`.
pub const PEM_LINE_ENDING: LineEnding = LineEnding::LF;
