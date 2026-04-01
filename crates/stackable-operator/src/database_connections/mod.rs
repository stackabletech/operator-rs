use snafu::Snafu;

pub mod databases;
pub mod drivers;
mod helpers;
#[cfg(test)]
mod tests;

/// Aggregates errors from database-specific submodules.
///
/// All variants use `context(false)` to enable automatic [`From`] conversion,
/// so callers can use `?` directly without needing `.context(SomeSnafu)`.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false), display("PostgreSQL database connection error"))]
    Postgresql {
        source: databases::postgresql::Error,
    },

    #[snafu(context(false), display("MySQL database connection error"))]
    Mysql { source: databases::mysql::Error },

    #[snafu(context(false), display("Derby database connection error"))]
    Derby { source: databases::derby::Error },
}

/// Templating mechanism to use when substituting env variables.
///
/// Most products consume config files, which are templated using
/// [`config-utils`](https://github.com/stackabletech/config-utils). This is the recommended
/// mechanism, hence it's the default.
///
/// And than there is Airflow, where we configured everything via env variables, so that doesn't
/// work. So we also support using bash env substitution.
/// As of 2026-04 to my knowledge this is the only operator doing such, it would be great to
/// switch airflow-operator to config files, so that we can remove templating support for bash env
/// substitution.
#[derive(Copy, Clone, Debug, Default)]
pub enum TemplatingMechanism {
    /// Template files using <https://github.com/stackabletech/config-utils>, e.g.
    /// `${env:EXAMPLE_USERNAME}`
    #[default]
    ConfigUtils,

    /// Let `bash` substitute the env variable, e.g. `${EXAMPLE_USERNAME}`.
    BashEnvSubstitution,
}
