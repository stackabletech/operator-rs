use std::path::PathBuf;

use clap::Parser;

use crate::file_types::FileType;

/// Fill out variables in config files from either env variables or files directly.
#[derive(Debug, Parser)]
pub struct TemplateCommand {
    /// The path to the file that should be templated
    pub file: PathBuf,

    /// The optional file type of the file to be templated. If this is not specified this utility will try to infer
    /// the type based on the file name.
    #[arg(value_enum)]
    pub file_type: Option<FileType>,

    /// By default inserted values are automatically escaped according to the deteced file format. You can disable
    /// this, e.g. when you need to insert XML tags (as they otherwise would be escaped).
    /// NOTE: Please make sure to correctly escape the inserted text on your own!
    #[clap(long)]
    pub dont_escape: bool,
}
