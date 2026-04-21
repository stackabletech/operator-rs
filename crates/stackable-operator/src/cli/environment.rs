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

    /// The image registry which should be used when resolving images provisioned by the operator.
    ///
    /// Example values include: `127.0.0.1` or `oci.example.org`.
    ///
    /// Note that when running the operator on Kubernetes we recommend to provide this value via
    /// the deployment mechanism, like Helm.
    #[arg(long, env, value_parser = url::Host::parse)]
    pub image_registry: url::Host,

    /// The image repository used in conjunction with the `image_registry` to form the final image
    /// name.
    ///
    /// Example values include: `airflow-operator` or `path/to/hbase-operator`.
    ///
    /// Note that when running the operator on Kubernetes we recommend to provide this value via
    /// the deployment mechanism, like Helm. Additionally, care must be taken when this value is
    /// used as part of the product image selection, as it (most likely) includes the `-operator`
    /// suffix.
    #[arg(long, env)]
    pub image_repository: String,
}
