use clap::Parser;
use config_utils::templating::{self, template};
use snafu::{ResultExt, Snafu};

use args::{Args, Command};

mod args;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to template file"))]
    TemplateFile { source: templating::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[snafu::report]
fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Template { file, file_type } => {
            template(&file, file_type.as_ref()).context(TemplateFileSnafu)?;
        }
    }

    Ok(())
}
