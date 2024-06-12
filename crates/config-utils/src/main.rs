use clap::Parser;
use config_utils::template::{self, template};
use snafu::{ResultExt, Snafu};

use cli_args::{Args, Command};

mod cli_args;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to template file"))]
    TemplateFile { source: template::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[snafu::report]
fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Template {
            file,
            file_type,
            dont_escape,
        } => {
            template(&file, file_type.as_ref(), !dont_escape).context(TemplateFileSnafu)?;
        }
    }

    Ok(())
}
