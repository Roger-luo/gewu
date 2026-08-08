//! Command-line runtime for Gewu.

use std::process::ExitCode;

/// Runs the Gewu command-line application.
#[derive(Debug, Default)]
pub struct Runtime;

impl Runtime {
    #[must_use]
    pub fn run(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}
