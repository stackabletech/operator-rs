use clap::{Parser, Subcommand};

use config_utils::template::cli_args::TemplateCommand;

/// Utility that helps you handling config files.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Template(TemplateCommand),
}
