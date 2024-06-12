pub mod file_types;
pub mod template;

pub const ENV_VAR_PATTERN_START: &str = "${env:";
pub const ENV_VAR_PATTERN_END: &str = "}";

pub const FILE_PATTERN_START: &str = "${file:UTF-8:";
pub const FILE_PATTERN_END: &str = "}";
