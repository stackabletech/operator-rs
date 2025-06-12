use clap::{Parser, Subcommand};
use snafu::{ResultExt, Snafu};

mod crd;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("failed to generate CRD previews"))]
    Crd { source: crd::Error },
}

#[derive(Debug, Parser)]
enum Command {
    #[command(subcommand)]
    Crd(CrdCommand),
}

#[derive(Debug, Subcommand)]
enum CrdCommand {
    Preview,
}

#[snafu::report]
fn main() -> Result<(), Error> {
    let command = Command::parse();

    match command {
        Command::Crd(crd_command) => match crd_command {
            CrdCommand::Preview => crd::generate_preview().context(CrdSnafu),
        },
    }
}
