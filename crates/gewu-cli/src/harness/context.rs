use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use gewu_review::{Quote, ReviewInput, context::ContextReader};

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[arg(long, default_value = ".gewu/review-input.json")]
    input: PathBuf,
    /// Hard limit for the complete compact JSON response, including metadata.
    #[arg(long, default_value_t = 16_384)]
    max_bytes: usize,
    /// The previous response's `next` object, encoded as JSON.
    #[arg(long)]
    cursor: Option<String>,
    #[command(subcommand)]
    command: ContextCommand,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    /// Page through all change IDs, paths, sizes and hashes, without patches.
    Inventory,
    /// Find literal text in the frozen corpus; returns bounded source references.
    Search {
        #[arg(long)]
        query: String,
    },
    /// Read a hunk-body span, retaining its original ID and coordinates.
    Source {
        #[arg(long)]
        change_id: String,
        #[arg(long)]
        start: usize,
        #[arg(long)]
        end: usize,
        /// Print each body line with the number a quote uses and the file line
        /// the reviewer's gutter shows, instead of JSON.
        /// For reading while planning; the JSON form is the bounded protocol.
        #[arg(long)]
        numbered: bool,
    },
}

impl ContextArgs {
    pub fn run(self) -> Result<()> {
        let input: ReviewInput = serde_json::from_slice(
            &fs::read(&self.input).with_context(|| format!("read {}", self.input.display()))?,
        )?;
        let reader = ContextReader::new(&input, self.max_bytes);
        let output = match self.command {
            ContextCommand::Search { query } => {
                let cursor = self
                    .cursor
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?;
                serde_json::to_string(&reader.search(&query, cursor.as_ref())?)?
            }
            ContextCommand::Inventory => {
                let cursor = self
                    .cursor
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?;
                serde_json::to_string(&reader.inventory(cursor.as_ref())?)?
            }
            ContextCommand::Source {
                change_id,
                start,
                end,
                numbered: true,
            } => {
                let change = input
                    .changes
                    .iter()
                    .find(|change| change.id == change_id)
                    .with_context(|| format!("unknown change id `{change_id}`"))?;
                let end = end.min(change.body_lines());
                anyhow::ensure!(start >= 1 && start <= end, "no body lines {start}-{end}");
                // Body numbers are what quotes use; the file line is what the
                // reviewer's gutter shows and what prose should cite. Removed
                // lines have none.
                change
                    .body(start, end)
                    .iter()
                    .zip(start..)
                    .map(|(text, line)| {
                        let file = change
                            .new_line_at(line)
                            .map_or_else(String::new, |file| file.to_string());
                        format!("{line:>4} {file:>5} │{text}\n")
                    })
                    .collect()
            }
            ContextCommand::Source {
                change_id,
                start,
                end,
                numbered: false,
            } => {
                let cursor = self
                    .cursor
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?;
                serde_json::to_string(&reader.source(
                    &Quote {
                        change_id,
                        start,
                        end,
                    },
                    cursor.as_ref(),
                )?)?
            }
        };
        // No pretty-printing or trailing newline: stdout is exactly the bytes
        // whose bound was checked by the domain layer.
        print!("{output}");
        Ok(())
    }
}
