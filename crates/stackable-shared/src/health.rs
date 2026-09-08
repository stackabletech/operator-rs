use std::{
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A single named check contributing to one health endpoint.
///
/// A check only passes once [`HealthCheck::mark_passed`] has been called.
#[derive(Clone)]
pub struct HealthCheck {
    name: String,
    passed: Arc<AtomicBool>,
}

impl HealthCheck {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn mark_passed(&self) {
        self.passed.store(true, Ordering::Release);
    }

    fn passed(&self) -> bool {
        self.passed.load(Ordering::Acquire)
    }
}

/// A set of checks to be used for a health endpoint a probe can call.
///
/// # Example
///
/// ```
/// use stackable_shared::health::HealthCheckRegistry;
///
/// let mut startup_checks = HealthCheckRegistry::new();
/// let crds_established = startup_checks.register("crds-established");
///
/// assert!(!startup_checks.all_passed());
/// crds_established.mark_passed();
/// assert!(startup_checks.all_passed());
/// ```
#[derive(Default)]
pub struct HealthCheckRegistry {
    checks: Vec<HealthCheck>,
}

impl HealthCheckRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new [`HealthCheck`] with the provided name and returns it.
    pub fn register(&mut self, name: impl Into<String>) -> HealthCheck {
        let check = HealthCheck::new(name);
        self.checks.push(check.clone());
        check
    }

    /// Returns `true` if all the registered health checks have passed or no health checks are
    /// registered.
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(HealthCheck::passed)
    }
}

impl Display for HealthCheckRegistry {
    /// Renders one line per check, with the check's name and status only. Anything else, error
    /// causes in particular, must not end up in a response to an unauthenticated endpoint.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.checks.is_empty() {
            return writeln!(f, "[ok] no checks registered");
        }

        for check in &self.checks {
            let status = if check.passed() { "ok" } else { "pending" };
            writeln!(f, "[{status}] {name}", name = check.name)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn passed_on_empty_registry() {
        let registry = HealthCheckRegistry::new();

        assert!(registry.all_passed());
    }

    #[test]
    fn passed_only_once_every_check_is() {
        let mut registry = HealthCheckRegistry::new();
        let crds = registry.register("crds-established");
        let migration = registry.register("database-migrated");

        assert!(!registry.all_passed());

        crds.mark_passed();
        assert!(!registry.all_passed());

        migration.mark_passed();
        assert!(registry.all_passed());
    }
}
