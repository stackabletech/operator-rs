pub mod file_types;
pub mod template;

// It could be the case that the colon (:) in the start pattern has been escaped, as e.g. the product-config crate does
pub const ENV_VAR_START_PATTERNS: [&str; 2] = ["${env:", "${env\\:"];
pub const ENV_VAR_END_PATTERN: &str = "}";

// It could be the case that the colon (:) in the start pattern has been escaped, as e.g. the product-config crate does
pub const FILE_START_PATTERNS: [&str; 2] = ["${file:UTF-8:", "${file\\:UTF-8\\:"];
pub const FILE_END_PATTERN: &str = "}";
