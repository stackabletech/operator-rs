/// Health checks and health check registry for health endpoints used by probes
///
/// The naming here follows the Kubernetes convention of a health check / health check registry and
/// their usage by the different probes.
///
/// ## References
///
/// - <https://github.com/kubernetes/kubernetes/blob/master/staging/src/k8s.io/apiserver/pkg/server/healthz/healthz.go#L41>
/// - <https://github.com/kubernetes/kubernetes/blob/master/staging/src/k8s.io/apiserver/pkg/server/healthz.go#L34>
/// - <https://github.com/kubernetes/kubernetes/blob/master/staging/src/k8s.io/apiserver/pkg/server/genericapiserver.go#L205>
/// - <https://github.com/kubernetes/kubernetes/blob/master/staging/src/k8s.io/apiserver/pkg/server/healthz/healthz.go#L330>
use std::{
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// A single named check contributing to one health endpoint.
///
/// A check can be marked as passing with a call to [`HealthCheck::mark_passed`], and reset with
/// [`HealthCheck::mark_not_passed`], e.g. for a liveness check that can start failing again.
#[derive(Clone)]
pub struct HealthCheck {
    name: String,
    // This has to be an AtomicBool as we could otherwise not share references to it.
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

    pub fn mark_not_passed(&self) {
        self.passed.store(false, Ordering::Release);
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
/// use stackable_webhook::health::HealthCheckRegistry;
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
    /// Creates a new [`HealthCheckRegistry`] with no health checks registered.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Registers a new [`HealthCheck`] with the provided name and returns it.
    ///
    /// The returned [`HealthCheck`] can be used to mark the check as passed.
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

impl IntoResponse for &HealthCheckRegistry {
    fn into_response(self) -> Response {
        let status = if self.all_passed() {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        // The response body carries check names and their status. Error causes etc. go to the
        // log, never into a response to not leak internal information to the public endpoint.
        (status, self.to_string()).into_response()
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
mod tests {
    use super::*;

    #[test]
    fn passed_on_empty_registry() {
        let registry = HealthCheckRegistry::new();

        assert!(registry.all_passed());
    }

    #[test]
    fn passed_only_once_every_check_passed() {
        let mut registry = HealthCheckRegistry::new();
        let crds = registry.register("crds-established");
        let migration = registry.register("database-migrated");

        assert!(!registry.all_passed());

        crds.mark_passed();
        assert!(!registry.all_passed());

        migration.mark_passed();
        assert!(registry.all_passed());
    }

    #[test]
    fn not_passed_after_check_marked_not_passed() {
        let mut registry = HealthCheckRegistry::new();
        let crds = registry.register("crds-established");

        crds.mark_passed();
        assert!(registry.all_passed());

        crds.mark_not_passed();
        assert!(!registry.all_passed());
    }
}
