use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config_utils::file_types::FileType;

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

        /// By default inserted values are automatically escaped according to the deteced file format. You can disable
        /// this, e.g. when you need to insert XML tags (as they otherwise would be escaped).
        /// NOTE: Please make sure to correctly escape the inserted text on your own!
        #[clap(long)]
        dont_escape: bool,
    },
}
