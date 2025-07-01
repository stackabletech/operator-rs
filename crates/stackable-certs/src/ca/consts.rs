use stackable_operator::time::Duration;

/// The default CA validity time span of one hour (3600 seconds).
pub const DEFAULT_CA_VALIDITY: Duration = Duration::from_hours_unchecked(1);

/// The root CA subject name containing only the common name.
pub const SDP_ROOT_CA_SUBJECT: &str = "CN=Stackable Data Platform Internal CA";
