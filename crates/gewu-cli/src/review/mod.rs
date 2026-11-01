mod audit;
mod feedback;
mod frames;
mod handover;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use gewu_review::{
    ChangePresentation, ChunkBlock, ChunkKind, EventLog, Overview, Quote, ReviewChunk, ReviewInput,
    ReviewSession, ReviewSpec, ValidatedReview, ValidationErrors,
};

use crate::{git::Repository, tui::ReviewTui};

use self::audit::AuditArgs;
use self::feedback::{ResolveArgs, RetargetArgs};
use self::frames::FramesArgs;
use self::handover::HandoverArgs;

const GUIDE: &str = include_str!("../../../../harness/review.md");

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Create an immutable manifest for an agent.
    Prepare(PrepareArgs),
    /// Print the portable agent guide.
    Guide,
    /// Validate an agent-generated review specification.
    Validate(ValidateArgs),
    /// Check a validated review's coverage and references mechanically.
    Audit(AuditArgs),
    /// Generate a deterministic file-ordered fallback specification.
    Fallback(FallbackArgs),
    /// Have an agent write the walkthrough, stopping at each reviewer gate.
    Generate(crate::harness::GenerateArgs),
    /// Review a GitHub pull request: `gewu review pr 123 --agent claude`.
    Pr(PullRequestArgs),
    /// Open a validated review in the terminal UI.
    Open(OpenArgs),
    /// Render every page of a review headlessly and check what a machine can.
    Frames(FramesArgs),
    /// Print what the reviewer produced, for whoever acts on it next.
    Handover(HandoverArgs),
    /// Mark feedback addressed, so the harness stops holding for it.
    Resolve(ResolveArgs),
    /// Re-aim feedback at the code or at the walkthrough.
    Retarget(RetargetArgs),
}

impl ReviewCommand {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Prepare(args) => args.run(),
            Self::Guide => {
                print!("{GUIDE}");
                Ok(())
            }
            Self::Validate(args) => args.run(),
            Self::Audit(args) => args.run(),
            Self::Fallback(args) => args.run(),
            Self::Generate(args) => args.run(),
            Self::Pr(args) => args.generate.for_pull_request(args.number).run(),
            Self::Open(args) => args.run(),
            Self::Frames(args) => args.run(),
            Self::Handover(args) => args.run(),
            Self::Resolve(args) => args.run(),
            Self::Retarget(args) => args.run(),
        }
    }
}

#[derive(Debug, Args)]
pub struct PullRequestArgs {
    /// The pull request's number.
    number: u64,
    #[command(flatten)]
    generate: crate::harness::GenerateArgs,
}

#[derive(Debug, Args)]
pub struct PrepareArgs {
    #[arg(long, default_value = "HEAD^")]
    base: String,
    #[arg(long, default_value = "HEAD")]
    head: String,
    #[arg(long, default_value = ".gewu/review-input.json")]
    output: PathBuf,
}

impl PrepareArgs {
    fn run(self) -> Result<()> {
        let repository = Repository::new(Path::new("."));
        let input = repository.review_input(
            &self.base,
            &self.head,
            gewu_review::DEFAULT_MAX_CHUNK_LINES,
        )?;
        repository.ensure_changes(&input, false)?;
        write_json(&self.output, &input)?;
        println!(
            "wrote {} changes to {}\nrun `gewu review guide` in your chosen agent",
            input.changes.len(),
            self.output.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct FallbackArgs {
    #[arg(long, default_value = ".gewu/review-input.json")]
    input: PathBuf,
    #[arg(long, default_value = ".gewu/review-spec.json")]
    output: PathBuf,
}

impl FallbackArgs {
    fn run(self) -> Result<()> {
        let input: ReviewInput = read_json(&self.input)?;
        let chunks = input
            .changes
            .iter()
            .enumerate()
            .map(|(index, change)| ReviewChunk {
                id: format!("fallback-{}", index + 1),
                title: format!("Review {}", change.path),
                summary: "Raw diff in file order. No walkthrough was generated, so this chunk \
                          has no explanation; `gewu review generate` writes one."
                    .into(),
                kind: ChunkKind::Other,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                    presentation: ChangePresentation::Diff,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
                section: None,
            })
            .collect();
        let spec = ReviewSpec::new(
            input.source,
            Overview {
                intent: "Review the selected Git changes.".into(),
                changes: vec!["Using deterministic file-ordered fallback chunks.".into()],
                illustration: None,
                concepts: vec![],
            },
            chunks,
        );
        write_json(&self.output, &spec)?;
        println!("wrote fallback review spec to {}", self.output.display());
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[command(flatten)]
    artifacts: ArtifactArgs,
    /// Accept a review that is still being expanded one work item at a time.
    #[arg(long)]
    partial: bool,
}

impl ValidateArgs {
    fn run(self) -> Result<()> {
        let review = if self.partial {
            self.artifacts.load_partial()?
        } else {
            self.artifacts.load()?
        };
        let covered: std::collections::HashSet<_> = review
            .spec()
            .chunks
            .iter()
            .flat_map(ReviewChunk::change_ids)
            .collect();
        println!(
            "valid review spec {} · {} of {} changes covered",
            review.spec().session_key(),
            covered.len(),
            review.input().changes.len()
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Args)]
pub struct ArtifactArgs {
    #[arg(long, default_value = ".gewu/review-input.json")]
    input: PathBuf,
    #[arg(long, default_value = ".gewu/review-spec.json")]
    spec: PathBuf,
}

impl ArtifactArgs {
    fn load(&self) -> Result<ValidatedReview> {
        self.checked(ValidatedReview::validate)
    }

    /// Reviews are opened while the harness is still expanding work items, so
    /// unexpanded changes must not block the reviewer.
    fn load_partial(&self) -> Result<ValidatedReview> {
        self.checked(ValidatedReview::validate_partial)
    }

    fn checked(
        &self,
        validate: fn(ReviewInput, ReviewSpec) -> Result<ValidatedReview, ValidationErrors>,
    ) -> Result<ValidatedReview> {
        let input: ReviewInput = read_json(&self.input)?;
        let spec: ReviewSpec = read_json(&self.spec)?;
        validate(input, spec).map_err(|errors| {
            let details = errors
                .0
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n- ");
            anyhow::anyhow!("{errors}\n- {details}")
        })
    }
}

/// The artifacts plus the reviewer's own record of working through them.
#[derive(Clone, Debug, Args)]
pub struct SessionArgs {
    #[command(flatten)]
    artifacts: ArtifactArgs,
    #[arg(long)]
    session: Option<PathBuf>,
}

impl SessionArgs {
    /// Where the reviewer's events live. Derived from the review's own id, so
    /// appending chunks or fixing a typo cannot orphan their progress.
    fn path(&self, review: &ValidatedReview) -> PathBuf {
        self.session.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                ".gewu/reviews/{}/events.jsonl",
                review.spec().session_key()
            ))
        })
    }

    fn load(&self) -> Result<(ValidatedReview, EventLog, ReviewSession)> {
        let review = self.artifacts.load_partial()?;
        let log = EventLog::new(self.path(&review));
        let session = log.load()?;
        Ok((review, log, session))
    }
}

#[derive(Debug, Args)]
pub struct OpenArgs {
    #[command(flatten)]
    session: SessionArgs,
}

impl OpenArgs {
    fn run(self) -> Result<()> {
        let review = self.session.artifacts.load_partial()?;
        let path = self.session.path(&review);
        ReviewTui::new(review, path)?.run()
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let contents = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_slice(&contents).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_vec_pretty(value)?;
    fs::write(path, contents).with_context(|| format!("could not write {}", path.display()))
}
