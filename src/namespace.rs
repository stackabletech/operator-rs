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

pub fn foo() {
    match env::var(WATCH_NAMESPACE_ENV) {
        Ok(var) => {}
        Err(_) => {}
    }
}

pub fn parse_watch_namespaces(watch_namespace: String) -> Option<Vec<String>> {
    if watch_namespace.is_empty() {
        return None;
    }

    let split = watch_namespace.split(",");
    Some(split.collect())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_watch_namespaces() {
        let result = parse_watch_namespaces("foobar");
        println!("{:?}", result);
    }
}
