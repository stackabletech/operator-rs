mod config;
mod validating;

pub use config::*;
pub use validating::*;

#[derive(Debug, Default, strum::Display)]
pub enum SideEffects {
    #[default]
    None,
    NoneOnDryRun,
}
