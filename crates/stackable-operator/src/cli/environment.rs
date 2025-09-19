#[derive(Debug, PartialEq, Eq, clap::Parser)]
#[command(next_help_heading = "Environment Options")]
pub struct OperatorEnvironmentOptions {
    /// The namespace the operator is running in, usually `stackable-operators`.
    ///
    /// Note that when running the operator on Kubernetes we recommend to use the
    /// [downward API](https://kubernetes.io/docs/concepts/workloads/pods/downward-api/)
    /// to let Kubernetes project the namespace as the `OPERATOR_NAMESPACE` env variable.
    #[arg(long, env)]
    pub operator_namespace: String,

    /// The name of the service the operator is reachable at, usually
    /// something like `<product>-operator`.
    #[arg(long, env)]
    pub operator_service_name: String,
}
