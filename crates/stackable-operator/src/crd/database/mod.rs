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

// /// TODO docs
// pub trait CeleryDatabaseConnection {
//     /// TODO docs, e.g. on what are valid characters for unique_database_name
//     fn celery_connection_details(
//         &self,
//         unique_database_name: &str,
//     ) -> CeleryDatabaseConnectionDetails;
// }

// pub struct CeleryDatabaseConnectionDetails {
//     /// The connection URI, which can contain env variable templates, e.g.
//     /// `redis://:redis@airflow-redis-master:6379/0`
//     pub uri_template: String,

//     /// The [`EnvVar`]s the operator needs to mount into the created Pods.
//     pub env_vars: Vec<EnvVar>,
// }

// impl CeleryDatabaseConnectionDetails {
//     pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
//         cb.add_env_vars(self.env_vars.clone());
//     }
// }
