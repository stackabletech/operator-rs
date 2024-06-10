use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config_filler::file_types::FileType;

/// Utility to fill out missing variables in config files
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Template {
        /// The path to the file that should be templated
        file: PathBuf,

        /// The optional file type of the file to be templated. If this is not specified this utility will try to infer
        /// the type based on the file name.
        #[arg(value_enum)]
        file_type: Option<FileType>,
    },
}
