use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkContext, ChunkKind, Decision, Feedback,
    FeedbackKind, Overview, Quote, ReviewChunk, ReviewSession, ValidatedReview,
};
use serde::{Deserialize, Serialize};

use crate::FileView;

/// Everything the UI needs to render the current review.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub review_id: String,
    pub source: SourceView,
    pub overview: Overview,
    /// Every file the review covers, whether or not a step shows it yet.
    pub files: Vec<FileView>,
    pub generation: GenerationView,
    pub chunks: Vec<ChunkView>,
    pub feedback: Vec<Feedback>,
    pub summary: SummaryView,
    pub last_visited: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceView {
    pub base: String,
    pub head: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GenerationView {
    pub remaining_work_items: Option<usize>,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChunkView {
    pub id: String,
    pub index: usize,
    pub title: String,
    pub summary: String,
    pub kind: ChunkKind,
    pub section: Option<String>,
    pub confidence: Option<String>,
    pub oversized_reason: Option<String>,
    pub context: Option<ChunkContext>,
    pub blocks: Vec<BlockView>,
    pub status: ChunkStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChunkStatus {
    pub visited: bool,
    pub reviewed: bool,
    pub open_issues: usize,
    pub feedback_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockView {
    Narrative {
        markdown: String,
    },
    Changes {
        presentation: ChangePresentation,
        quotes: Vec<QuoteView>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QuoteView {
    pub change_id: String,
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub language: Option<String>,
    pub more_above: bool,
    pub more_below: bool,
    pub lines: Vec<QuoteLine>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QuoteLine {
    pub kind: QuoteLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteLineKind {
    Added,
    Removed,
    Context,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummaryView {
    pub reviewed: usize,
    pub total: usize,
    pub unreviewed: Vec<String>,
    pub unresolved_issues: Vec<String>,
    pub decision: Option<Decision>,
    pub suggested_decision: Decision,
    pub batch_scoped: bool,
}

impl Snapshot {
    pub(crate) fn from_review(review: &ValidatedReview, session: &ReviewSession) -> Self {
        let spec = review.spec();
        let changes: std::collections::HashMap<&str, &Change> = review
            .input()
            .changes
            .iter()
            .map(|change| (change.id.as_str(), change))
            .collect();
        let chunk_ids: Vec<_> = spec.chunks.iter().map(|chunk| chunk.id.clone()).collect();
        let remaining = spec.remaining_work_items();
        let complete = remaining.is_none_or(|count| count == 0);
        let chunks: Vec<_> = spec
            .chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| ChunkView::from_chunk(index, chunk, session, &changes))
            .collect();
        let reviewed = chunks.iter().filter(|chunk| chunk.status.reviewed).count();
        let unreviewed: Vec<_> = chunks
            .iter()
            .filter(|chunk| !chunk.status.reviewed)
            .map(|chunk| chunk.id.clone())
            .collect();
        let unresolved_issues: Vec<_> = session
            .live_unresolved_issues(chunk_ids.iter())
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
        let last_visited = session
            .last_visited
            .as_ref()
            .filter(|id| chunk_ids.iter().any(|chunk| chunk == *id))
            .cloned();
        Self {
            review_id: spec.session_key(),
            source: SourceView {
                base: spec.source.base.clone(),
                head: spec.source.head.clone(),
            },
            overview: spec.overview.clone(),
            files: FileView::collect(review.input(), &chunks),
            generation: GenerationView {
                remaining_work_items: remaining,
                complete,
            },
            feedback: session.ordered().cloned().collect(),
            summary: SummaryView {
                reviewed,
                total: chunks.len(),
                unreviewed,
                unresolved_issues,
                decision: session.decision,
                suggested_decision: session.outcome(chunk_ids.iter()),
                batch_scoped: !complete,
            },
            chunks,
            last_visited,
        }
    }
}

impl ChunkView {
    fn from_chunk(
        index: usize,
        chunk: &ReviewChunk,
        session: &ReviewSession,
        changes: &std::collections::HashMap<&str, &Change>,
    ) -> Self {
        let feedback = session.chunk_feedback(&chunk.id);
        let open_issues = feedback
            .iter()
            .filter(|item| item.kind == FeedbackKind::Issue && !item.resolved)
            .count();
        Self {
            id: chunk.id.clone(),
            index,
            title: chunk.title.clone(),
            summary: chunk.summary.clone(),
            kind: chunk.kind,
            section: chunk.section.clone(),
            confidence: chunk.confidence.clone(),
            oversized_reason: chunk.oversized_reason.clone(),
            context: chunk.context.clone(),
            blocks: chunk
                .content
                .iter()
                .map(|block| BlockView::from_block(block, changes))
                .collect(),
            status: ChunkStatus {
                visited: session.visited.contains(&chunk.id),
                reviewed: session.reviewed.contains(&chunk.id),
                open_issues,
                feedback_count: feedback.len(),
            },
        }
    }
}

impl BlockView {
    fn from_block(block: &ChunkBlock, changes: &std::collections::HashMap<&str, &Change>) -> Self {
        match block {
            ChunkBlock::Narrative { markdown } => Self::Narrative {
                markdown: markdown.clone(),
            },
            ChunkBlock::Changes {
                quotes,
                presentation,
            } => Self::Changes {
                presentation: *presentation,
                quotes: quotes
                    .iter()
                    .filter_map(|quote| resolve_quote(quote, changes))
                    .collect(),
            },
        }
    }
}

fn resolve_quote(
    quote: &Quote,
    changes: &std::collections::HashMap<&str, &Change>,
) -> Option<QuoteView> {
    changes
        .get(quote.change_id.as_str())
        .map(|change| QuoteView::resolve(quote, change))
}
