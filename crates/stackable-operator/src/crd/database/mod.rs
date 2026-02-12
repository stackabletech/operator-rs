use snafu::Snafu;

pub mod databases;
pub mod drivers;
mod helpers;
#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(context(false), display("postgresql error"))]
    Postgresql {
        source: databases::postgresql::Error,
    },

    #[snafu(context(false), display("derby error"))]
    Derby { source: databases::derby::Error },
}
