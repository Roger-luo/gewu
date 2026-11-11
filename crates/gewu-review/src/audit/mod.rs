//! Mechanical checks on a finished walkthrough, for evaluating generation.
//!
//! Validation proves every line is shown. An audit asks the next questions a
//! machine can still answer exactly: whether the review's scope reached the
//! manifest, whether every shown line sits beside prose that names something
//! in it, and whether the code the prose names exists. Whether that prose is
//! true, minimal, or useful remains a judgement for an independent reviewer.

mod boundaries;
mod citations;
mod coverage;
mod echoes;
mod prose;
mod references;
mod symbols;

use serde::Serialize;

use crate::{ChunkBlock, ChunkContext, ReviewChunk, ValidatedReview};

pub use boundaries::SplitSentence;
pub use citations::InvisibleLine;
pub use citations::invisible_in;
pub use coverage::{ChangeCoverage, CoverageReport, LineRange};
pub use echoes::{DanglingTitle, Echo, dangling_titles, echoes};
pub use references::{DistantReference, ForwardReference, ReferenceReport, UnresolvedReference};
pub use symbols::SymbolIndex;

/// Chunks between showing a reviewed definition and leaning on it again
/// before the reader is likely to need it re-shown.
pub const DEFAULT_ANCHOR_DISTANCE: usize = 5;

/// An audit of one validated review.
#[derive(Debug)]
pub struct Audit<'a> {
    review: &'a ValidatedReview,
    symbols: SymbolIndex,
    scope: Option<Vec<String>>,
    anchor_distance: usize,
}

impl<'a> Audit<'a> {
    #[must_use]
    pub fn new(review: &'a ValidatedReview, symbols: SymbolIndex) -> Self {
        Self {
            review,
            symbols,
            scope: None,
            anchor_distance: DEFAULT_ANCHOR_DISTANCE,
        }
    }

    /// The paths the reviewer asked to review. Each must reach the manifest.
    #[must_use]
    pub fn with_scope(mut self, paths: Vec<String>) -> Self {
        self.scope = Some(paths);
        self
    }

    #[must_use]
    pub fn with_anchor_distance(mut self, chunks: usize) -> Self {
        self.anchor_distance = chunks.max(1);
        self
    }

    #[must_use]
    pub fn run(&self) -> AuditReport {
        AuditReport {
            coverage: CoverageReport::new(self.review, self.scope.as_deref()),
            references: ReferenceReport::new(self.review, &self.symbols, self.anchor_distance),
            split_sentences: boundaries::check(self.review),
            echoes: echoes(self.review.spec(), &[], &self.review.spec().chunks),
            dangling_titles: dangling_titles(
                &self
                    .review
                    .spec()
                    .chunks
                    .iter()
                    .map(|chunk| chunk.title.as_str())
                    .collect::<Vec<_>>(),
                &self.review.spec().chunks,
                &self.review.input().changes,
            ),
            chunks: self
                .review
                .spec()
                .chunks
                .iter()
                .map(|chunk| ChunkStats::new(self.review, chunk))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditReport {
    pub coverage: CoverageReport,
    pub references: ReferenceReport,
    /// Reported for the judge, not gated: an overlapping view may bridge it.
    pub split_sentences: Vec<SplitSentence>,
    /// Sentences that mostly repeat earlier text; reported for the judge.
    pub echoes: Vec<Echo>,
    /// Title references that name no chunk; reported for the judge.
    pub dangling_titles: Vec<DanglingTitle>,
    pub chunks: Vec<ChunkStats>,
}

impl AuditReport {
    /// Forward and distant references are reported for a judge to weigh;
    /// only provable defects fail the audit.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.coverage.passes() && self.references.passes()
    }
}

/// Size of a chunk's prose against what it shows. A trend to watch across
/// generations, not a threshold: some lines need a paragraph, most need none.
#[derive(Clone, Debug, Serialize)]
pub struct ChunkStats {
    pub chunk_id: String,
    pub prose_words: usize,
    pub shown_lines: usize,
}

impl ChunkStats {
    fn new(review: &ValidatedReview, chunk: &ReviewChunk) -> Self {
        Self {
            chunk_id: chunk.id.clone(),
            prose_words: ChunkProse::new(chunk).text().split_whitespace().count(),
            shown_lines: coverage::shown_lines(review, chunk),
        }
    }
}

/// The reader-facing text of one chunk, apart from its context pane.
struct ChunkProse<'a> {
    parts: Vec<&'a str>,
}

impl<'a> ChunkProse<'a> {
    fn new(chunk: &'a ReviewChunk) -> Self {
        let mut parts = vec![chunk.title.as_str(), chunk.summary.as_str()];
        parts.extend(chunk.content.iter().flat_map(|block| match block {
            ChunkBlock::Narrative { markdown } => vec![markdown.as_str()],
            ChunkBlock::Changes { .. } => Vec::new(),
            ChunkBlock::Explainer(explainer) => explainer.prose(),
        }));
        parts.extend(chunk.confidence.as_deref());
        Self { parts }
    }

    fn text(&self) -> String {
        self.parts.join("\n")
    }

    fn context(chunk: &ReviewChunk) -> Vec<&str> {
        match &chunk.context {
            None => vec![],
            Some(ChunkContext::Concept {
                definition,
                example,
                relation,
            }) => vec![definition, example, relation],
            Some(ChunkContext::Refactor {
                before,
                after,
                reason,
            }) => vec![before, after, reason],
        }
    }
}
