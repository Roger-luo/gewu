use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use gewu_review::{ReviewInput, harness::WorkList};

use super::artifacts;

#[derive(Debug, Args)]
pub struct ValidatePlanArgs {
    #[arg(long, default_value = ".gewu/review-input.json")]
    input: PathBuf,
    #[arg(long)]
    plan: PathBuf,
    /// Repeat for a local planning scope. Omit to validate the whole manifest.
    #[arg(long)]
    change_id: Vec<String>,
    #[arg(long, default_value_t = super::DEFAULT_MAX_CHUNKS)]
    max_chunks: usize,
}

impl ValidatePlanArgs {
    pub fn run(self) -> Result<()> {
        let input: ReviewInput = artifacts::read(self.input)?;
        let list: WorkList = artifacts::read(self.plan)?;
        let result = if self.change_id.is_empty() {
            list.validate_for(&input, self.max_chunks)
        } else {
            list.validate_scope(&input, &self.change_id, self.max_chunks)
        };
        result.map_err(|errors| {
            anyhow::anyhow!(
                "{}",
                errors
                    .0
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })?;
        println!("valid plan: {} work items", list.items.len());
        Ok(())
    }
}
