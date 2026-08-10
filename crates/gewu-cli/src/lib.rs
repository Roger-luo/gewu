//! Command-line runtime for Gewu.

mod git;
mod harness;
mod review;
mod tui;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Runs the Gewu command-line application.
#[derive(Debug, Default)]
pub struct Runtime;

impl Runtime {
    #[must_use]
    pub fn run(self) -> ExitCode {
        match Cli::parse().command.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error:#}");
                ExitCode::FAILURE
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "gewu", about = "Human-guided code review")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Progressively guide an agent to generate a review spec.
    Harness {
        #[command(subcommand)]
        command: harness::HarnessCommand,
    },
    /// Prepare, validate, and open code reviews.
    Review {
        #[command(subcommand)]
        command: review::ReviewCommand,
    },
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Harness { command } => command.run(),
            Self::Review { command } => command.run(),
        }
    }
}
