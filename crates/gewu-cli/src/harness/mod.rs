mod context;
mod generate;
mod research;
mod validate_plan;
mod watch;

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};
use gewu_git::Repository;
use gewu_harness::{DEFAULT_MAX_CHUNKS, HarnessRunner};
use gewu_review::harness::ReviewOrigin;

pub use self::generate::GenerateArgs;

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
                let runner = HarnessRunner::new(args.max_chunks, false)
                    .with_proofread(args.proofread_rounds);
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
    /// Hold chunks to this many seconds of estimated reading instead of rows,
    /// as the desktop app does (its default is the rubric's, see `gewu
    /// review rubric`).
    #[arg(long)]
    max_chunk_seconds: Option<u32>,
    /// Chunks one work item may expand into.
    #[arg(long, default_value_t = DEFAULT_MAX_CHUNKS)]
    max_chunks: usize,
    /// Score each expanded batch against the rubric before the reviewer reads
    /// it, allowing this many rewrites. A fresh agent should do the scoring;
    /// `gewu review generate` arranges that and proofreads by default.
    #[arg(long, default_value_t = 0)]
    proofread_rounds: u32,
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
            max_chunk_seconds: None,
            max_chunks: DEFAULT_MAX_CHUNKS,
            proofread_rounds: 0,
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
        let input = input.with_max_chunk_seconds(self.max_chunk_seconds);
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
