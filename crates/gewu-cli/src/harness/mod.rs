mod artifacts;
mod context;
mod estimate;
mod generate;
mod prompts;
mod research;
mod research_state;
mod runner;
mod validate_plan;
mod watch;

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};
use gewu_review::harness::ReviewOrigin;

use crate::git::Repository;

pub use self::generate::GenerateArgs;
use self::runner::HarnessRunner;

/// Chunks one expansion may put in front of a reviewer at once.
pub const DEFAULT_MAX_CHUNKS: usize = 6;

#[derive(Debug, Subcommand)]
pub enum HarnessCommand {
    /// Fresh expansion instructions without repeating the full investigation.
    ExpansionGuide(ContinueArgs),
    /// Measure and validate a candidate without advancing the review.
    MeasureExpansion {
        #[arg(long)]
        draft: std::path::PathBuf,
        #[arg(long, default_value_t = DEFAULT_MAX_CHUNKS)]
        max_chunks: usize,
    },
    /// Validate a local or merged plan without changing harness state.
    ValidatePlan(validate_plan::ValidatePlanArgs),
    /// Investigate bounded source pages with verified evidence and durable progress.
    Research(research::ResearchArgs),
    /// Read bounded inventory or source pages without advancing the harness.
    Context(context::ContextArgs),
    /// Start a progressive agent review-generation run.
    Start(StartArgs),
    /// Validate the current checkpoint and print the next prompt.
    Next(ContinueArgs),
    /// Reprint the current prompt without advancing.
    Status(ContinueArgs),
    /// Remove generated harness state and artifacts.
    Reset,
}

impl HarnessCommand {
    pub fn run(self) -> Result<()> {
        let prompt = match self {
            Self::ExpansionGuide(args) => {
                HarnessRunner::new(args.max_chunks, false).expansion_guide()?
            }
            Self::MeasureExpansion { draft, max_chunks } => {
                let result = HarnessRunner::new(max_chunks, false).measure_expansion(&draft)?;
                print!("{}", serde_json::to_string(&result)?);
                return Ok(());
            }
            Self::ValidatePlan(args) => return args.run(),
            Self::Research(args) => return args.run(),
            Self::Context(args) => return args.run(),
            Self::Start(args) => {
                let runner = HarnessRunner::new(args.max_chunks, false);
                let (review_id, input, origin) = args.begin()?;
                runner.start(review_id, input, origin)?
            }
            Self::Next(args) => HarnessRunner::new(args.max_chunks, args.force).next()?,
            Self::Status(args) => HarnessRunner::new(args.max_chunks, args.force).status()?,
            Self::Reset => HarnessRunner::new(DEFAULT_MAX_CHUNKS, false).reset()?,
        };
        print!("{prompt}");
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct ContinueArgs {
    /// Chunks one work item may expand into.
    #[arg(long, default_value_t = DEFAULT_MAX_CHUNKS)]
    max_chunks: usize,
    /// Expand the next item even while change requests are unresolved.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
pub struct StartArgs {
    #[arg(long, default_value = "HEAD^")]
    base: String,
    #[arg(long)]
    head: Option<String>,
    #[arg(long)]
    working_tree: bool,
    /// Rendered rows a chunk may occupy. Raise it only to ask deliberately for
    /// larger chunks than one terminal viewport.
    #[arg(long, default_value_t = gewu_review::DEFAULT_MAX_CHUNK_LINES)]
    max_chunk_lines: usize,
    /// Chunks one work item may expand into.
    #[arg(long, default_value_t = DEFAULT_MAX_CHUNKS)]
    max_chunks: usize,
}

impl StartArgs {
    /// A start with the default budgets, for commands that start a run on
    /// the reviewer's behalf.
    pub(super) fn new(base: String, head: Option<String>, working_tree: bool) -> Self {
        Self {
            base,
            head,
            working_tree,
            max_chunk_lines: gewu_review::DEFAULT_MAX_CHUNK_LINES,
            max_chunks: DEFAULT_MAX_CHUNKS,
        }
    }

    fn begin(self) -> Result<(String, gewu_review::ReviewInput, ReviewOrigin)> {
        let repository = Repository::new(Path::new("."));
        let input = if self.working_tree {
            repository.worktree_input(&self.base, self.max_chunk_lines)
        } else {
            repository.review_input(
                &self.base,
                self.head.as_deref().unwrap_or("HEAD"),
                self.max_chunk_lines,
            )
        }?;
        repository.ensure_changes(&input, self.working_tree)?;
        let origin = ReviewOrigin {
            base: self.base,
            head: self.head,
            working_tree: self.working_tree,
        };
        Ok((review_id()?, input, origin))
    }
}

/// Identifies the run itself, so a reviewer's session survives every later
/// edit to the spec.
fn review_id() -> Result<String> {
    Ok(format!(
        "review-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ))
}
