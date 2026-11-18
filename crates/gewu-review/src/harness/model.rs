use serde::{Deserialize, Serialize};

use super::WriteAhead;
use crate::{Quote, ReviewChunk, ReviewInput, SourceIdentity};

use super::{
    HARNESS_SCHEMA_VERSION, HarnessValidationErrors, Proofreading, validation::CheckpointValidator,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessStage {
    /// Understand the whole change set once.
    Investigate,
    /// Rewrite chunks the reviewer objected to.
    Revise,
    /// Turn the investigation into an ordered work list.
    Plan,
    /// Expand the topmost unexpanded item into chunks.
    Expand,
    /// Repair an expansion that failed validation.
    Repair,
    /// Score the batch just expanded against the proofread rubric.
    Proofread,
    /// Rewrite the chunks the proofread scored too low.
    Polish,
    /// Wait for the human to review what has been expanded.
    Await,
    /// Every item is expanded and reviewed.
    Complete,
}

/// How the manifest was produced, so the harness can re-diff and notice the
/// reviewer's edits without being told about them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewOrigin {
    pub base: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub working_tree: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HarnessState {
    pub schema_version: u32,
    pub review_id: String,
    pub source: SourceIdentity,
    pub origin: ReviewOrigin,
    pub stage: HarnessStage,
    pub revision: u32,
    /// Once enabled, missing research artifacts cannot silently select the
    /// legacy investigation path, including after a source-driven replan.
    #[serde(default)]
    pub research_required: bool,
    /// Chunks the reviewer asked to have rewritten, while the `Revise` stage
    /// runs. A rewrite must replace exactly these, so the harness remembers
    /// what it asked for rather than re-deriving it from a session that the
    /// reviewer may have moved on in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revising: Vec<String>,
    /// Rewrites a batch may get after proofreading before it goes to the
    /// reviewer anyway. Zero skips proofreading.
    #[serde(default)]
    pub proofread_rounds: u32,
    /// The chunks of the batch being proofread, and the rewrites spent on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proofreading: Option<Proofreading>,
    /// The older switch for writing ahead: on meant one batch. Read through
    /// [`HarnessState::write_ahead`], which prefers `ahead`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub write_ahead: bool,
    /// How far the agent may write ahead of the reviewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead: Option<WriteAhead>,
    /// Has the agent explain items of tests. Off, Gewu writes them itself.
    /// Runs started before this existed had every item explained.
    #[serde(default = "explained", skip_serializing_if = "is_explained")]
    pub explain_tests: bool,
}

fn explained() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_explained(explain: &bool) -> bool {
    *explain
}

impl HarnessState {
    #[must_use]
    pub fn new(review_id: impl Into<String>, source: SourceIdentity, origin: ReviewOrigin) -> Self {
        Self {
            schema_version: HARNESS_SCHEMA_VERSION,
            review_id: review_id.into(),
            source,
            origin,
            stage: HarnessStage::Investigate,
            revision: 0,
            research_required: false,
            revising: Vec::new(),
            proofread_rounds: 0,
            proofreading: None,
            write_ahead: false,
            ahead: None,
            explain_tests: true,
        }
    }

    pub fn advance(&mut self, stage: HarnessStage) {
        if stage != HarnessStage::Revise {
            self.revising.clear();
        }
        if !matches!(stage, HarnessStage::Proofread | HarnessStage::Polish) {
            self.proofreading = None;
        }
        self.stage = stage;
        self.revision = self.revision.saturating_add(1);
    }

    /// Enters the rewrite stage, recording which chunks it owes.
    pub fn revise(&mut self, chunk_ids: Vec<String>) {
        self.advance(HarnessStage::Revise);
        self.revising = chunk_ids;
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InvestigationFinding {
    pub change_ids: Vec<String>,
    pub summary: String,
    /// Exact original body spans backing this finding; empty for legacy notes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_spans: Vec<Quote>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InvestigationCheckpoint {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub intent: String,
    /// Three to five behavioral bullets for the review's first screen.
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub concepts: Vec<String>,
    /// A map of how the parts of the change connect, for the overview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illustration: Option<crate::Illustration>,
    pub findings: Vec<InvestigationFinding>,
}

impl InvestigationCheckpoint {
    pub fn validate(&self, input: &ReviewInput) -> Result<(), HarnessValidationErrors> {
        CheckpointValidator::investigation(self, input)
    }
}

/// One unit of review work: a coherent slice of the change set that has been
/// planned but not necessarily expanded into chunks yet.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkItem {
    pub id: String,
    pub title: String,
    pub purpose: String,
    /// The spans of the diff this item is responsible for walking through.
    /// Items may overlap where the story needs the same lines twice.
    pub covers: Vec<Quote>,
    #[serde(default)]
    pub expanded: bool,
    /// The chunks written for this item, so the gate can tell the batch the
    /// reviewer may still be reading from the ones they have finished.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<String>,
    /// The item only walks through tests. Unless the run explains tests,
    /// Gewu writes its chunks itself from the test code, with no agent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tests: bool,
}

impl WorkItem {
    pub fn change_ids(&self) -> impl Iterator<Item = &String> {
        self.covers.iter().map(|quote| &quote.change_id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkList {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub items: Vec<WorkItem>,
}

impl WorkList {
    /// Validate an independently authored subset without rebasing any source
    /// IDs or line coordinates. A merged plan still requires full validation.
    pub fn validate_scope(
        &self,
        input: &ReviewInput,
        change_ids: &[String],
        max_chunks: usize,
    ) -> Result<(), HarnessValidationErrors> {
        use super::HarnessValidationError;
        if change_ids.is_empty() {
            return Err(HarnessValidationErrors(vec![
                HarnessValidationError::Empty("planning scope"),
            ]));
        }
        let unknown: Vec<_> = change_ids
            .iter()
            .filter(|id| !input.changes.iter().any(|change| &change.id == *id))
            .map(|id| HarnessValidationError::UnknownChange(id.clone()))
            .collect();
        if !unknown.is_empty() {
            return Err(HarnessValidationErrors(unknown));
        }
        let subset = ReviewInput {
            schema_version: input.schema_version,
            source: input.source.clone(),
            max_chunk_lines: input.max_chunk_lines,
            max_chunk_seconds: input.max_chunk_seconds,
            changes: input
                .changes
                .iter()
                .filter(|change| change_ids.contains(&change.id))
                .cloned()
                .collect(),
        };
        self.validate_for(&subset, max_chunks)
    }

    pub fn validate(&self, input: &ReviewInput) -> Result<(), HarnessValidationErrors> {
        CheckpointValidator::work_list(self, input)
    }

    /// Validates the plan against the expansion budget chosen by the caller.
    ///
    /// A single large change may later use a compact presentation such as
    /// `summary_only`. A multi-change item, however, must leave enough room to
    /// walk its ordinary source without making the expansion checkpoint
    /// impossible to satisfy.
    pub fn validate_for(
        &self,
        input: &ReviewInput,
        max_chunks: usize,
    ) -> Result<(), HarnessValidationErrors> {
        CheckpointValidator::work_list_for(self, input, max_chunks)
    }

    /// The item the agent should work on next.
    #[must_use]
    pub fn next_item(&self) -> Option<&WorkItem> {
        self.items.iter().find(|item| !item.expanded)
    }

    /// Marks an item expanded into `chunks`.
    pub fn mark_expanded(&mut self, id: &str, chunks: &[String]) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.expanded = true;
            item.chunks = chunks.to_vec();
        }
    }

    /// The chunks of the batch written last: the last expanded item's. Empty
    /// when none is recorded, as in plans written before items kept them.
    #[must_use]
    pub fn latest_batch(&self) -> &[String] {
        self.items
            .iter()
            .rev()
            .find(|item| item.expanded)
            .map_or(&[], |item| item.chunks.as_slice())
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.items.iter().filter(|item| !item.expanded).count()
    }

    /// Drops unexpanded items so a fresh plan can replace them, keeping the
    /// record of what has already been expanded.
    pub fn retain_expanded(&mut self) {
        self.items.retain(|item| item.expanded);
    }
}

/// Chunks produced for one work item, awaiting append to the review spec.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Expansion {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub item_id: String,
    pub chunks: Vec<ReviewChunk>,
}

impl Expansion {
    pub fn validate(
        &self,
        input: &ReviewInput,
        item: &WorkItem,
        max_chunks: usize,
    ) -> Result<(), HarnessValidationErrors> {
        CheckpointValidator::expansion(self, input, item, max_chunks)
    }
}

/// Rewritten chunks, replacing the ones a reviewer objected to.
///
/// A rewrite keeps the chunk ids it replaces and the lines they showed. The
/// reviewer's feedback is filed against a chunk id, so a rewrite that renamed
/// or dropped one would orphan the very request it answers; and the spec's
/// coverage of the manifest is an invariant that a rewrite must not spend.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Revision {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub chunks: Vec<ReviewChunk>,
}

impl Revision {
    pub fn validate(
        &self,
        input: &ReviewInput,
        replacing: &[&ReviewChunk],
    ) -> Result<(), HarnessValidationErrors> {
        CheckpointValidator::revision(self, input, replacing)
    }
}
