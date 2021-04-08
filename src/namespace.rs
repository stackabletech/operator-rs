//! This module provides helpers and constants to deal with namespaces
//!
use crate::error::{Error, OperatorResult};
use crate::validation::validate_namespace_name;
use std::env;
use std::env::VarError;

// The default namespace which is applied when not specified by clients
pub const NAMESPACE_DEFAULT: &str = "default";
pub const NAMESPACE_ALL: &str = "";

/// The system namespace where we place system components.
pub const NAMESPACE_SYSTEM: &str = "kube-system";

/// The namespace where we place public info (ConfigMaps).
pub const NAMESPACE_PUBLIC: &str = "kube-public";

pub const WATCH_NAMESPACE_ENV: &str = "WATCH_NAMESPACE";

pub enum WatchNamespace {
    All,
    One(String),
}

/// This gets the namespace to watch for an Operator.
///
/// This uses the environment variable `WATCH_NAMESPACE` and partially follows the Go operator-sdk:
/// * If the variable is not defined or empty (i.e. `""`") we'll watch _all_ namespaces.
/// * If the variable is set it must be a valid namespace
/// * If the variable contains invalid unicode we'll return an error
/// * If the variable contains an invalid namespace name we'll return an error
///
/// This differs from the operator-sdk in that we only allow a _single namespace_ at the moment.
/// operator-sdk supports multiple comma-separated namespaces.
pub fn get_watch_namespace() -> OperatorResult<WatchNamespace> {
    match env::var(WATCH_NAMESPACE_ENV) {
        Ok(var) if var.is_empty() => Ok(WatchNamespace::All),
        Ok(var) if !validate_namespace_name(&var, false).is_empty() => {
            let errors = validate_namespace_name(&var, false);
            Err(Error::InvalidName { errors })
        }
        Ok(var) => Ok(WatchNamespace::One(var)),
        Err(VarError::NotPresent) => Ok(WatchNamespace::All),
        Err(err) => Err(Error::EnvironmentVariableError { source: err }),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_watch_namespaces() {
        assert!(matches!(get_watch_namespace(), Ok(WatchNamespace::All)));

        let test_value = "foo".to_string();
        env::set_var(WATCH_NAMESPACE_ENV, &test_value);
        assert!(
            matches!(get_watch_namespace(), Ok(WatchNamespace::One(value)) if value == test_value)
        );

        env::set_var(WATCH_NAMESPACE_ENV, "");
        assert!(matches!(get_watch_namespace(), Ok(WatchNamespace::All)));

        env::set_var(WATCH_NAMESPACE_ENV, "0");
        assert!(matches!(
            get_watch_namespace(),
            Err(Error::InvalidName { .. })
        ));
    }
}
