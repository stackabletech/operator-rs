use tracing;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub mod controller;
mod k8s_events;

#[derive(Debug, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum TracingTarget {
    None,
    Jaeger,
}

impl Default for TracingTarget {
    fn default() -> Self {
        Self::None
    }
}

/// Initializes `tracing` logging with options from the environment variable
/// given in the `env` parameter.
///
/// We force users to provide a variable name so it can be different per product.
/// We encourage it to be the product name plus `_LOG`, e.g. `FOOBAR_OPERATOR_LOG`.
/// If no environment variable is provided, the maximum log level is set to INFO.
pub fn initialize_logging(env: &str, app_name: &str, tracing_target: TracingTarget) {
    let filter = match EnvFilter::try_from_env(env) {
        Ok(env_filter) => env_filter,
        _ => EnvFilter::try_new(tracing::Level::INFO.to_string())
            .expect("Failed to initialize default tracing level to INFO"),
    };

    let fmt = tracing_subscriber::fmt::layer();
    let registry = Registry::default().with(filter).with(fmt);

    match tracing_target {
        TracingTarget::None => registry.init(),
        TracingTarget::Jaeger => {
            let jaeger = opentelemetry_jaeger::new_agent_pipeline()
                .with_service_name(app_name)
                .install_batch(opentelemetry::runtime::Tokio)
                .expect("Failed to initialize Jaeger pipeline");
            let opentelemetry = tracing_opentelemetry::layer().with_tracer(jaeger);
            registry.with(opentelemetry).init();
        }
    }
}

#[cfg(test)]
mod test {

    use tracing::{debug, error, info};

    use crate::logging::TracingTarget;

    // If there is a proper way to programmatically inspect the global max level than we should use that.
    // Until then, this is mostly a sanity check for the implementation above.
    // Either run
    //      cargo test default_tracing -- --nocapture
    // to see the ERROR and INFO messages, or
    //      NOT_SET=debug cargo test default_tracing -- --nocapture
    // to see them all.
    #[test]
    pub fn test_default_tracing_level_is_set_to_info() {
        super::initialize_logging("NOT_SET", "test", TracingTarget::None);

        error!("ERROR level messages should be seen.");
        info!("INFO level messages should also be seen by default.");
        debug!("DEBUG level messages should be seen only if you set the NOT_SET env var.");
    }
}
