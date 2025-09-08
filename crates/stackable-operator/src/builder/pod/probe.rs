//! Kubernetes [`Probe`] builder.
//!
//! The upstream [`Probe`] struct does not prevent invalid probe configurations
//! which leads to surprises at runtime which can be deeply hidden.
//! You need to specify at least an action and interval (in this order).
//!
//! ### Usage example
//!
//! ```
//! use stackable_operator::{
//!     builder::pod::probe::ProbeBuilder,
//!     shared::time::Duration,
//! };
//! # use k8s_openapi::api::core::v1::HTTPGetAction;
//! # use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
//!
//! let probe = ProbeBuilder::http_get_port_scheme_path(8080, None, None)
//!     .with_period(Duration::from_secs(10))
//!     .build()
//!     .expect("failed to build probe");
//!
//! assert_eq!(
//!     probe.http_get,
//!     Some(HTTPGetAction {
//!         port: IntOrString::Int(8080),
//!         ..Default::default()
//!     })
//! );
//! assert_eq!(probe.period_seconds, Some(10));
//! ```

use std::num::TryFromIntError;

use k8s_openapi::{
    api::core::v1::{ExecAction, GRPCAction, HTTPGetAction, Probe, TCPSocketAction},
    apimachinery::pkg::util::intstr::IntOrString,
};
use snafu::{ResultExt, Snafu, ensure};
use stackable_shared::time::Duration;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display(
        "The probe's {field:?} duration of {duration} is too long, as it's seconds doesn't fit into an i32"
    ))]
    DurationTooLong {
        source: TryFromIntError,
        field: String,
        duration: Duration,
    },

    #[snafu(display("The probe period is zero, but it needs to be a positive duration"))]
    PeriodIsZero {},
}

#[derive(Clone, Debug)]
pub struct ProbeBuilder<Action, Period> {
    // Mandatory field
    action: Action,
    period: Period,

    // Fields with defaults
    success_threshold: i32,
    failure_threshold: i32,
    timeout: Duration,
    initial_delay: Duration,
    termination_grace_period: Option<Duration>,
}

/// Available probes
///
/// Only one probe can be configured at a time. For more details about each
/// type, see [container-probes] documentation.
///
/// [container-probes]: https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#container-probes
#[derive(Clone, Debug)]
pub enum ProbeAction {
    Exec(ExecAction),
    Grpc(GRPCAction),
    HttpGet(HTTPGetAction),
    TcpSocket(TCPSocketAction),
}

impl ProbeBuilder<(), ()> {
    /// This probe action executes the specified command
    pub fn exec_command(
        command: impl IntoIterator<Item = impl Into<String>>,
    ) -> ProbeBuilder<ProbeAction, ()> {
        Self::exec(ExecAction {
            command: Some(command.into_iter().map(Into::into).collect()),
        })
    }

    // Note: Ideally we also have a builder for `HTTPGetAction`, but that is lot's of effort we
    // don't want to spend now.
    /// This probe action does an HTTP GET request to the specified port. Optionally, you can
    /// configure a scheme and path, otherwise the Kubernetes default is used.
    pub fn http_get_port_scheme_path(
        port: u16,
        scheme: Option<String>,
        path: Option<String>,
    ) -> ProbeBuilder<ProbeAction, ()> {
        Self::http_get(HTTPGetAction {
            path,
            scheme,
            port: IntOrString::Int(port.into()),
            ..Default::default()
        })
    }

    /// Set's an [`ExecAction`] as probe.
    ///
    /// You likely want to use [`Self::exec_command`] whenever possible.
    pub fn exec(exec_action: ExecAction) -> ProbeBuilder<ProbeAction, ()> {
        Self::action(ProbeAction::Exec(exec_action))
    }

    /// Set's an [`GRPCAction`] as probe.
    pub fn grpc(grpc_action: GRPCAction) -> ProbeBuilder<ProbeAction, ()> {
        Self::action(ProbeAction::Grpc(grpc_action))
    }

    /// Set's an [`HTTPGetAction`] as probe.
    ///
    /// For simple cases, there is a a convenience helper: [`Self::http_get_port_scheme_path`].
    pub fn http_get(http_get_action: HTTPGetAction) -> ProbeBuilder<ProbeAction, ()> {
        Self::action(ProbeAction::HttpGet(http_get_action))
    }

    /// Set's an [`TCPSocketAction`] as probe.
    pub fn tcp_socket(tcp_socket_action: TCPSocketAction) -> ProbeBuilder<ProbeAction, ()> {
        Self::action(ProbeAction::TcpSocket(tcp_socket_action))
    }

    /// Incase you already have an [`ProbeAction`] enum variant you can pass that here.
    ///
    /// If not, it is recommended to use one of the specialized functions such as [`Self::exec`],
    /// [`Self::grpc`], [`Self::http_get`] or [`Self::tcp_socket`] or their helper functions.
    pub fn action(action: ProbeAction) -> ProbeBuilder<ProbeAction, ()> {
        ProbeBuilder {
            action,
            period: (),
            // The following values match the Kubernetes defaults
            success_threshold: 1,
            failure_threshold: 1,
            timeout: Duration::from_secs(1),
            initial_delay: Duration::from_secs(0),
            termination_grace_period: None,
        }
    }
}

impl ProbeBuilder<ProbeAction, ()> {
    /// The period/interval in which the probe should be executed.
    pub fn with_period(self, period: Duration) -> ProbeBuilder<ProbeAction, Duration> {
        let Self {
            action,
            period: (),
            success_threshold,
            failure_threshold,
            timeout,
            initial_delay,
            termination_grace_period,
        } = self;

        ProbeBuilder {
            action,
            period,
            success_threshold,
            failure_threshold,
            timeout,
            initial_delay,
            termination_grace_period,
        }
    }
}

impl ProbeBuilder<ProbeAction, Duration> {
    /// How often the probe must succeed before being considered successful.
    pub fn with_success_threshold(mut self, success_threshold: i32) -> Self {
        self.success_threshold = success_threshold;
        self
    }

    /// The duration the probe needs to succeed before being considered successful.
    ///
    /// This internally calculates the needed failure threshold based on the period and passes that
    /// to [`Self::with_success_threshold`].
    ///
    /// This function returns an [`Error::PeriodIsZero`] error in case the period is zero, as it
    /// can not divide by zero.
    pub fn with_success_threshold_duration(
        self,
        success_threshold_duration: Duration,
    ) -> Result<Self, Error> {
        ensure!(self.period.as_nanos() != 0, PeriodIsZeroSnafu);

        // SAFETY: Period is checked above to be non-zero
        let success_threshold = success_threshold_duration.div_duration_f32(*self.period);
        Ok(self.with_success_threshold(success_threshold.ceil() as i32))
    }

    /// After a probe fails `failureThreshold` times in a row, Kubernetes considers that the
    /// overall check has failed: the container is not ready/healthy/live.
    ///
    /// Minimum value is 1 second. For the case of a startup or liveness probe, if at least
    /// `failureThreshold` probes have failed, Kubernetes treats the container as unhealthy and
    /// triggers a restart for that specific container. The kubelet honors the setting of
    /// `terminationGracePeriodSeconds` for that container. For a failed readiness probe, the
    /// kubelet continues running the container that failed checks, and also continues to run more
    /// probes; because the check failed, the kubelet sets the `Ready` condition on the Pod to
    /// `false`.
    pub fn with_failure_threshold(mut self, failure_threshold: i32) -> Self {
        self.failure_threshold = failure_threshold;
        self
    }

    /// Number of seconds after which the probe times out.
    ///
    /// Minimum value is 1 second.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Number of seconds after the container has started before startup, liveness or readiness
    /// probes are initiated.
    ///
    /// If a startup probe is defined, liveness and readiness probe delays do not begin until the
    /// startup probe has succeeded. If the value of periodSeconds is greater than
    /// `initialDelaySeconds` then the `initialDelaySeconds` will be ignored.
    pub fn with_initial_delay(mut self, initial_delay: Duration) -> Self {
        self.initial_delay = initial_delay;
        self
    }

    /// Configure a grace period for the kubelet to wait between triggering a shut down of the
    /// failed container, and then forcing the container runtime to stop that container.
    ///
    /// The default (if this function is not called) is to inherit the Pod-level value for
    /// `terminationGracePeriodSeconds` (30 seconds if not specified), and the minimum value is
    /// 1 second. See probe-level `terminationGracePeriodSeconds` for more detail.
    pub fn with_termination_grace_period(mut self, termination_grace_period: Duration) -> Self {
        self.termination_grace_period = Some(termination_grace_period);
        self
    }

    /// The duration the probe needs to fail before being considered failed.
    ///
    /// This internally calculates the needed failure threshold based on the period and passes that
    /// to [`Self::with_failure_threshold`].
    ///
    /// This function returns an [`Error::PeriodIsZero`] error in case the period is zero, as it
    /// can not divide by zero.
    pub fn with_failure_threshold_duration(
        self,
        failure_threshold_duration: Duration,
    ) -> Result<Self, Error> {
        ensure!(self.period.as_nanos() != 0, PeriodIsZeroSnafu);

        // SAFETY: Period is checked above to be non-zero
        let failure_threshold = failure_threshold_duration.div_duration_f32(*self.period);
        Ok(self.with_failure_threshold(failure_threshold.ceil() as i32))
    }

    /// Build the [`Probe`] using the specified contents.
    ///
    /// Returns an [`Error::DurationTooLong`] error in case the involved [`Duration`]s are too
    /// long.
    pub fn build(self) -> Result<Probe, Error> {
        let mut probe = Probe {
            exec: None,
            failure_threshold: Some(self.failure_threshold),
            grpc: None,
            http_get: None,
            initial_delay_seconds: Some(self.initial_delay.as_secs().try_into().context(
                DurationTooLongSnafu {
                    field: "initialDelay",
                    duration: self.initial_delay,
                },
            )?),
            period_seconds: Some(self.period.as_secs().try_into().context(
                DurationTooLongSnafu {
                    field: "period",
                    duration: self.period,
                },
            )?),
            success_threshold: Some(self.success_threshold),
            tcp_socket: None,
            termination_grace_period_seconds: match self.termination_grace_period {
                Some(termination_grace_period) => {
                    Some(termination_grace_period.as_secs().try_into().context(
                        DurationTooLongSnafu {
                            field: "terminationGracePeriod",
                            duration: termination_grace_period,
                        },
                    )?)
                }
                None => None,
            },
            timeout_seconds: Some(self.timeout.as_secs().try_into().context(
                DurationTooLongSnafu {
                    field: "timeout",
                    duration: self.timeout,
                },
            )?),
        };

        match self.action {
            ProbeAction::Exec(exec_action) => probe.exec = Some(exec_action),
            ProbeAction::Grpc(grpc_action) => probe.grpc = Some(grpc_action),
            ProbeAction::HttpGet(http_get_action) => probe.http_get = Some(http_get_action),
            ProbeAction::TcpSocket(tcp_socket_action) => probe.tcp_socket = Some(tcp_socket_action),
        }

        Ok(probe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_builder_minimal() {
        let probe = ProbeBuilder::http_get_port_scheme_path(8080, None, None)
            .with_period(Duration::from_secs(10))
            .build()
            .expect("Valid inputs must produce a Probe");

        assert_eq!(
            probe,
            Probe {
                exec: None,
                failure_threshold: Some(1),
                grpc: None,
                http_get: Some(HTTPGetAction {
                    port: IntOrString::Int(8080),
                    ..Default::default()
                }),
                initial_delay_seconds: Some(0),
                period_seconds: Some(10),
                success_threshold: Some(1),
                tcp_socket: None,
                termination_grace_period_seconds: None,
                timeout_seconds: Some(1),
            }
        );
    }

    #[test]
    fn test_probe_builder_complex() {
        let probe = ProbeBuilder::exec_command(["sleep", "1"])
            .with_period(Duration::from_secs(5))
            .with_success_threshold(2)
            .with_failure_threshold_duration(Duration::from_secs(33))
            .expect("The period is always non-zero")
            .with_timeout(Duration::from_secs(3))
            .with_initial_delay(Duration::from_secs(7))
            .with_termination_grace_period(Duration::from_secs(4))
            .build()
            .expect("Valid inputs must produce a Probe");

        assert_eq!(
            probe,
            Probe {
                exec: Some(ExecAction {
                    command: Some(vec!["sleep".to_owned(), "1".to_owned()])
                }),
                failure_threshold: Some(7),
                grpc: None,
                http_get: None,
                initial_delay_seconds: Some(7),
                period_seconds: Some(5),
                success_threshold: Some(2),
                tcp_socket: None,
                termination_grace_period_seconds: Some(4),
                timeout_seconds: Some(3),
            }
        );
    }
}
