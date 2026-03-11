//! Kubernetes Job lifecycle management for scaling hooks.
//!
//! [`JobTracker`] provides an idempotent mechanism for running a Kubernetes `Job` as part
//! of a scaling hook. It uses server-side apply to create the Job if absent, then checks
//! completion status on each reconcile.
//!
//! [`job_name`] produces deterministic, DNS-safe Job names so the tracker can find the
//! same Job across reconcile calls without external state.

use k8s_openapi::api::batch::v1::Job;
use kube::ResourceExt;
use snafu::{ResultExt, Snafu};
use tracing::{debug, info, warn};

use crate::{client::Client, crd::scaler::hooks::HookOutcome};

/// Derive a stable, DNS-safe Job name from a scaler name and a stage label.
///
/// The name is deterministic so `JobTracker` can find the same Job on requeue
/// without storing state externally.
///
/// # Parameters
///
/// - `scaler_name`: Name of the [`StackableScaler`](super::StackableScaler) resource.
/// - `stage`: A short label like `"pre-scale"` or `"post-scale"`.
///
/// # Returns
///
/// A DNS-safe string of at most 63 characters with no trailing hyphens.
pub fn job_name(scaler_name: &str, stage: &str) -> String {
    let raw = format!("{scaler_name}-{stage}");
    // Kubernetes resource names must be <= 63 chars and valid DNS labels.
    // Inputs are always ASCII (Kubernetes names), so byte-level truncation is safe.
    let truncated = &raw[..raw.len().min(63)];
    truncated.trim_end_matches('-').to_string()
}

/// Errors from managing a scaling hook [`Job`].
#[derive(Debug, Snafu)]
pub enum JobTrackerError {
    /// The server-side apply patch to create or update the Job failed.
    #[snafu(display("failed to apply Job '{job_name}'"))]
    ApplyJob {
        #[snafu(source(from(crate::client::Error, Box::new)))]
        source: Box<crate::client::Error>,
        /// The Kubernetes name of the Job that could not be applied.
        job_name: String,
    },
    /// The Job completed with one or more failed attempts.
    #[snafu(display("Job '{job_name}' failed: {message}"))]
    JobFailed {
        /// The Kubernetes name of the failed Job.
        job_name: String,
        /// Diagnostic message including the failure count and a `kubectl logs` hint.
        message: String,
    },
}

/// Manages the lifecycle of a Kubernetes [`Job`] used as a scaling hook.
///
/// Stateless -- the Job name is derived deterministically via [`job_name`],
/// so no persistent state is needed between reconciles.
pub struct JobTracker;

impl JobTracker {
    /// Ensures the Job exists (creates if absent via server-side apply), then checks completion.
    ///
    /// Returns:
    /// - `Ok(HookOutcome::Done)` — Job succeeded; Job is deleted as cleanup.
    /// - `Ok(HookOutcome::InProgress)` — Job is still running; requeue and re-call.
    /// - `Err(JobTrackerError::JobFailed)` -- Job failed; caller should transition to `Failed`.
    ///
    /// # Parameters
    ///
    /// - `client`: Kubernetes client for server-side apply, get, and delete operations.
    /// - `job`: The fully-constructed [`Job`] manifest. Its `.metadata.name` is used to
    ///   track it across reconcile calls -- use [`job_name`] to generate a stable name.
    /// - `namespace`: The namespace in which to manage the Job.
    pub async fn start_or_check(
        client: &Client,
        job: Job,
        namespace: &str,
    ) -> Result<HookOutcome, JobTrackerError> {
        let name = job.name_any();

        debug!(job = %name, namespace, "Applying hook Job (server-side apply)");

        // Apply (server-side apply — idempotent; no-op if Job already exists).
        // The response contains the full updated resource, so no separate GET is needed.
        let current: Job = client
            .apply_patch("stackable-operator", &job, &job)
            .await
            .context(ApplyJobSnafu {
                job_name: name.clone(),
            })?;

        let status = current.status.as_ref();
        let succeeded = status.and_then(|s| s.succeeded).unwrap_or(0);
        let failed = status.and_then(|s| s.failed).unwrap_or(0);

        if succeeded > 0 {
            info!(job = %name, namespace, "Hook Job completed successfully, cleaning up");
            // Best-effort cleanup — log errors but don't fail
            if let Err(e) = client.delete(&current).await {
                warn!(job = %name, namespace, error = %e, "Failed to clean up completed Job — it will be retried on next reconcile");
            }
            return Ok(HookOutcome::Done);
        }

        if failed > 0 {
            return Err(JobTrackerError::JobFailed {
                job_name: name.clone(),
                message: format!(
                    "{failed} attempt(s) failed — check pod logs with: kubectl logs -l job-name={name} -n {namespace}"
                ),
            });
        }

        debug!(job = %name, namespace, "Hook Job still running");
        Ok(HookOutcome::InProgress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_name_is_stable_for_same_inputs() {
        let name1 = job_name("my-scaler", "pre-scale");
        let name2 = job_name("my-scaler", "pre-scale");
        assert_eq!(name1, name2);
    }

    #[test]
    fn job_name_differs_for_different_stages() {
        let pre = job_name("my-scaler", "pre-scale");
        let post = job_name("my-scaler", "post-scale");
        assert_ne!(pre, post);
    }

    #[test]
    fn job_name_max_63_chars() {
        let long_name = "a".repeat(60);
        let name = job_name(&long_name, "pre-scale");
        assert!(name.len() <= 63, "name too long: {}", name.len());
    }

    #[test]
    fn job_name_no_trailing_hyphen() {
        // If truncation lands on a hyphen, it should be stripped
        let name = job_name("a".repeat(62).as_str(), "-x");
        assert!(!name.ends_with('-'));
    }

    #[test]
    fn job_name_format() {
        let name = job_name("my-cluster-default", "pre-scale");
        assert_eq!(name, "my-cluster-default-pre-scale");
    }
}
