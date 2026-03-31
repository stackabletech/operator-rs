use snafu::Snafu;

#[allow(clippy::module_inception)]
pub mod databases;
pub mod drivers;
mod helpers;
#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false), display("PostgreSQL error"))]
    Postgresql {
        source: databases::postgresql::Error,
    },

    #[snafu(context(false), display("MySQL error"))]
    Mysql { source: databases::mysql::Error },

    #[snafu(context(false), display("Derby error"))]
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
#[derive(Copy, Clone, Debug, Default)]
pub enum TemplatingMechanism {
    /// Template files using <https://github.com/stackabletech/config-utils>, e.g.
    /// `${env:EXAMPLE_USERNAME}`
    #[default]
    ConfigUtils,

    /// Let `bash` substitute the env variable, e.g. `${EXAMPLE_USERNAME}`.
    BashEnvSubstitution,
}
