use tracing_subscriber::EnvFilter;

/// Initializes `tracing` logging with options from the environment variable
/// given in the `env` parameter.
///
/// We force users to provide a variable name so it can be different per product.
/// We encourage it to be the product name plus `_LOG`, e.g. `FOOBAR_OPERATOR_LOG`.
pub fn initialize_logging(env: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env(env))
        .init();
}
