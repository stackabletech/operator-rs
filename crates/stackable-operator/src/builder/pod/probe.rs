use k8s_openapi::{
    api::core::v1::{ExecAction, GRPCAction, HTTPGetAction, Probe, TCPSocketAction},
    apimachinery::pkg::util::intstr::IntOrString,
};

use crate::time::Duration;

#[derive(Debug)]
pub struct ProbeBuilder<Action, Period> {
    action: Action,
    period: Period,

    success_threshold: i32,
    failure_threshold: i32,
    timeout: Duration,
    initial_delay: Duration,
    termination_grace_period: Duration,
}

impl Default for ProbeBuilder<(), ()> {
    fn default() -> Self {
        Self {
            action: (),
            period: (),
            // The following values match the Kubernetes default
            success_threshold: 1,
            failure_threshold: 1,
            timeout: Duration::from_secs(1),
            initial_delay: Duration::from_secs(0),
            termination_grace_period: Duration::from_secs(0),
        }
    }
}

pub enum ProbeAction {
    Exec(ExecAction),
    Grpc(GRPCAction),
    HttpGet(HTTPGetAction),
    TcpSocket(TCPSocketAction),
}

impl<Period> ProbeBuilder<(), Period> {
    /// This probe action executes the specified command
    pub fn with_exec_action_helper(
        self,
        command: impl IntoIterator<Item = impl Into<String>>,
    ) -> ProbeBuilder<ProbeAction, Period> {
        self.with_exec_action(ExecAction {
            command: Some(command.into_iter().map(Into::into).collect()),
        })
    }

    /// There is a convenience helper: [`Self::with_exec_action_helper`].
    pub fn with_exec_action(self, exec_action: ExecAction) -> ProbeBuilder<ProbeAction, Period> {
        self.with_action(ProbeAction::Exec(exec_action))
    }

    pub fn with_grpc_action(self, grpc_action: GRPCAction) -> ProbeBuilder<ProbeAction, Period> {
        self.with_action(ProbeAction::Grpc(grpc_action))
    }

    /// This probe action does an HTTP GET request to the specified port. Optionally, you can
    /// configure the path, otherwise the Kubernetes default is used.
    pub fn with_http_get_action_helper(
        self,
        port: u16,
        path: Option<impl Into<String>>,
    ) -> ProbeBuilder<ProbeAction, Period> {
        self.with_http_get_action(HTTPGetAction {
            path: path.map(Into::into),
            port: IntOrString::Int(port.into()),
            ..Default::default()
        })
    }

    /// There is a convenience helper: [`Self::with_http_get_action_helper`].
    pub fn with_http_get_action(
        self,
        http_get_action: HTTPGetAction,
    ) -> ProbeBuilder<ProbeAction, Period> {
        self.with_action(ProbeAction::HttpGet(http_get_action))
    }

    pub fn with_tcp_socket_action(
        self,
        tcp_socket_action: TCPSocketAction,
    ) -> ProbeBuilder<ProbeAction, Period> {
        self.with_action(ProbeAction::TcpSocket(tcp_socket_action))
    }

    /// Action-specific functions (e.g. [`Self::with_exec_action`] or [`Self::with_http_get_action`])
    /// are recommended instead.
    pub fn with_action(self, action: ProbeAction) -> ProbeBuilder<ProbeAction, Period> {
        let Self {
            action: (),
            period,
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

// success_threshold: i32,
// failure_threshold: i32,
// timeout: Duration,
// initial_delay: Duration,
// termination_grace_period: Duration,

impl ProbeBuilder<ProbeAction, Duration> {
    /// How often the probe must succeed before being considered successful.
    pub fn with_success_threshold(mut self, success_threshold: i32) -> Self {
        self.success_threshold = success_threshold;
        self
    }

    /// The duration the probe needs to succeed before being considered successful.
    ///
    /// This internally calculates the needed success threshold based on the period and passes that
    /// to [`Self::with_success_threshold`].
    pub fn with_success_threshold_duration(self, success_threshold_duration: Duration) -> Self {
        let success_threshold = success_threshold_duration.div_duration_f32(*self.period);
        // SAFETY: Returning an Result here would hurt the builder ergonomics and having such big
        // numbers does not have any real world effect.
        let success_threshold = success_threshold.ceil() as i32;
        self.with_success_threshold(success_threshold)
    }

    /// How often the probe must fail before being considered failed.
    pub fn with_failure_threshold(mut self, failure_threshold: i32) -> Self {
        self.failure_threshold = failure_threshold;
        self
    }

    /// The duration the probe needs to fail before being considered failed.
    ///
    /// This internally calculates the needed failure threshold based on the period and passes that
    /// to [`Self::with_failure_threshold`].
    pub fn with_failure_threshold_duration(self, failure_threshold_duration: Duration) -> Self {
        let failure_threshold = failure_threshold_duration.div_duration_f32(*self.period);
        // SAFETY: Returning an Result here would hurt the builder ergonomics and having such big
        // numbers does not have any real world effect.
        let failure_threshold = failure_threshold.ceil() as i32;
        self.with_failure_threshold(failure_threshold)
    }

    pub fn build(self) -> Probe {
        let mut probe = Probe {
            exec: None,
            failure_threshold: Some(self.failure_threshold),
            grpc: None,
            http_get: None,
            initial_delay_seconds: Some(
                self.initial_delay
                    .as_secs()
                    .try_into()
                    .expect("TODO Error handling"),
            ),
            period_seconds: Some(
                self.period
                    .as_secs()
                    .try_into()
                    .expect("TODO Error handling"),
            ),
            success_threshold: Some(self.success_threshold),
            tcp_socket: None,
            termination_grace_period_seconds: Some(
                self.termination_grace_period
                    .as_secs()
                    .try_into()
                    .expect("TODO Error handling"),
            ),
            timeout_seconds: Some(
                self.timeout
                    .as_secs()
                    .try_into()
                    .expect("TODO Error handling"),
            ),
        };

        match self.action {
            ProbeAction::Exec(exec_action) => probe.exec = Some(exec_action),
            ProbeAction::Grpc(grpc_action) => probe.grpc = Some(grpc_action),
            ProbeAction::HttpGet(http_get_action) => probe.http_get = Some(http_get_action),
            ProbeAction::TcpSocket(tcp_socket_action) => probe.tcp_socket = Some(tcp_socket_action),
        }

        probe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_builder() {
        let probe = ProbeBuilder::default()
            .with_exec_action_helper(["sleep", "1"])
            .with_period(Duration::from_secs(5))
            .with_failure_threshold_duration(Duration::from_secs(33))
            .build();

        assert_eq!(
            probe,
            Probe {
                exec: Some(ExecAction {
                    command: Some(vec!["sleep".to_owned(), "1".to_owned()])
                }),
                failure_threshold: Some(7),
                grpc: None,
                http_get: None,
                initial_delay_seconds: Some(0),
                period_seconds: Some(5),
                success_threshold: Some(1),
                tcp_socket: None,
                termination_grace_period_seconds: Some(0),
                timeout_seconds: Some(1),
            }
        );
    }
}
