use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Result, ensure};
use clap::{Args, Subcommand};
use gewu_review::{
    ReviewInput,
    research::{GroundedOverview, ResearchLedger, ResearchNotes, ResearchPolicy, ResearchRun},
};

use super::{artifacts, research_state};

#[derive(Debug, Args)]
pub struct ResearchArgs {
    #[arg(long, default_value = ".gewu/review-input.json")]
    input: PathBuf,
    #[command(subcommand)]
    command: ResearchCommand,
}

#[derive(Debug, Subcommand)]
enum ResearchCommand {
    /// Persist budgets before investigating. Never overwrites an existing run.
    Start {
        #[arg(long, default_value_t = 12_288)]
        page_bytes: usize,
        #[arg(long, default_value_t = 24_576)]
        request_bytes: usize,
        #[arg(long, default_value_t = 4096)]
        checkpoint_bytes: usize,
        #[arg(long, default_value_t = 256)]
        max_batches: usize,
    },
    /// Return the pending bounded author request, or null when research is complete.
    Next,
    /// Propose unchanged local notes from a validated archive; does not accept them.
    ReuseCandidate {
        #[arg(long)]
        previous_input: PathBuf,
        #[arg(long)]
        previous_ledger: PathBuf,
    },
    /// Validate and atomically save one response to the pending request.
    Accept {
        #[arg(long)]
        checkpoint: PathBuf,
    },
    /// Page through completed findings for synthesis; does not copy source patches.
    Findings {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 24_576)]
        max_bytes: usize,
    },
    /// Assemble a source-bound overview and all accepted findings for the harness.
    Assemble {
        #[arg(long)]
        overview: PathBuf,
    },
}

impl ResearchArgs {
    pub fn run(self) -> Result<()> {
        let input: ReviewInput = artifacts::read(&self.input)?;
        let root = self
            .input
            .parent()
            .ok_or_else(|| anyhow::anyhow!("input has no parent"))?
            .join("harness");
        let ledger_path = root.join("research.json");
        if let ResearchCommand::Start {
            page_bytes,
            request_bytes,
            checkpoint_bytes,
            max_batches,
        } = self.command
        {
            research_state::require_investigate(&root, &input)?;
            ensure!(
                !ledger_path.exists(),
                "research already exists; resume with research next"
            );
            let run = ResearchRun::new(
                &input,
                ResearchPolicy {
                    page_bytes,
                    request_bytes,
                    checkpoint_bytes,
                    max_batches,
                },
            )?;
            // Refuse impossible initial budgets before creating durable state.
            run.next()?;
            research_state::attach(&root)?;
            artifacts::write(&ledger_path, run.ledger())?;
            println!("research initialized; run `gewu harness research next`");
            return Ok(());
        }
        let ledger: ResearchLedger = artifacts::read(&ledger_path)?;
        let mut run = ResearchRun::resume(&input, ledger)?;
        match self.command {
            ResearchCommand::Start { .. } => unreachable!(),
            ResearchCommand::Next => print!("{}", serde_json::to_string(&run.next()?)?),
            ResearchCommand::ReuseCandidate {
                previous_input,
                previous_ledger,
            } => {
                let previous_input = artifacts::read(previous_input)?;
                let previous_ledger = artifacts::read(previous_ledger)?;
                print!(
                    "{}",
                    serde_json::to_string(&run.reuse_candidate(&previous_input, previous_ledger)?)?
                );
            }
            ResearchCommand::Accept { checkpoint } => {
                let notes: ResearchNotes = read_bounded(&checkpoint, run.checkpoint_byte_limit())?;
                run.accept(notes)?;
                artifacts::write(&ledger_path, run.ledger())?;
                println!("research notes accepted");
            }
            ResearchCommand::Findings { cursor, max_bytes } => {
                let cursor = cursor.as_deref().map(serde_json::from_str).transpose()?;
                print!(
                    "{}",
                    serde_json::to_string(&run.finding_page(cursor.as_ref(), max_bytes)?)?
                );
            }
            ResearchCommand::Assemble { overview } => {
                research_state::require_investigate(&root, &input)?;
                let overview: GroundedOverview =
                    read_bounded(&overview, run.checkpoint_byte_limit())?;
                let investigation = run.assemble(&overview)?;
                investigation.validate(&input)?;
                artifacts::write(root.join("research-overview.json"), &overview)?;
                artifacts::write(root.join("investigation.json"), &investigation)?;
                println!("investigation assembled; run `gewu harness next`");
            }
        }
        Ok(())
    }
}

fn read_bounded<T: serde::de::DeserializeOwned>(path: &Path, limit: usize) -> Result<T> {
    let mut data = Vec::new();
    File::open(path)?
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= limit, "checkpoint file exceeds {limit} bytes");
    Ok(serde_json::from_slice(&data)?)
}
