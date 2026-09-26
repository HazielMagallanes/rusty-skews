//! `rusty-skews-ctl`: diagnostics and control for rusty-skews.

mod doctor;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use skews_core::ControlCommand;

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
    /// Hide the bar if visible, show it if hidden.
    ToggleBar,
    /// Show the bar.
    ShowBar,
    /// Hide the bar.
    HideBar,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Doctor { strict: false }) {
        Command::Doctor { strict } => {
            let report = doctor::run(strict);
            print!("{}", report.text);
            std::process::exit(report.exit_code);
        }
        Command::ToggleBar => control(ControlCommand::ToggleBar),
        Command::ShowBar => control(ControlCommand::ShowBar),
        Command::HideBar => control(ControlCommand::HideBar),
    }
}

/// Sends a control command to the running shell.
fn control(command: ControlCommand) -> Result<()> {
    skews_ipc_control::send(command).context("failed to reach the running shell")
}
