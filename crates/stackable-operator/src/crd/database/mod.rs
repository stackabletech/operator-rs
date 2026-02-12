use snafu::Snafu;

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
