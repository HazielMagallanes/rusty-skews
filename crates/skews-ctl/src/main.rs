//! `rusty-skews-ctl`: diagnostics and control for rusty-skews.

mod doctor;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Command line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rusty-skews-ctl",
    version,
    about = "Diagnostics and control for rusty-skews"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Check whether this environment can run rusty-skews.
    Doctor {
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Doctor { strict: false }) {
        Command::Doctor { strict } => {
            let report = doctor::run(strict);
            print!("{}", report.text);
            std::process::exit(report.exit_code);
        }
    }
}
