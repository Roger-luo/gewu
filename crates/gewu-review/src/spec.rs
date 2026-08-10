use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{SCHEMA_VERSION, SourceIdentity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Illustration {
    BeforeAfter { before: String, after: String },
    Flow { steps: Vec<String> },
    Example { input: String, output: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Overview {
    pub intent: String,
    pub changes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illustration: Option<Illustration>,
    #[serde(default)]
    pub concepts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    Concept,
    Refactor,
    Integration,
    Test,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChunkContext {
    Concept {
        definition: String,
        example: String,
        relation: String,
    },
    Refactor {
        before: String,
        after: String,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangePresentation {
    Diff,
    AddedCode,
    RenderedMarkdown,
    SummaryOnly,
}

/// A span of one change's body, as the reviewer will see it.
///
/// `start` and `end` are inclusive 1-based line numbers within the hunk body —
/// the lines after its `@@` header. Spans may overlap, within a chunk and
/// across chunks: showing lines 10-50 after lines 1-25 because the reader needs
/// the earlier context again is good narrative, not a mistake.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Quote {
    pub change_id: String,
    pub start: usize,
    pub end: usize,
}

impl Quote {
    #[must_use]
    pub fn whole(change_id: impl Into<String>, lines: usize) -> Self {
        Self {
            change_id: change_id.into(),
            start: 1,
            end: lines.max(1),
        }
    }

    #[must_use]
    pub fn covers(&self, line: usize) -> bool {
        line >= self.start && line <= self.end
    }

    #[must_use]
    pub fn lines(&self) -> usize {
        self.end.saturating_sub(self.start) + 1
    }
}

/// One step inside a chunk. A chunk is an ordered sequence of these, so an
/// agent can explain a point, quote the lines that make it, and move on.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChunkBlock {
    Narrative {
        markdown: String,
    },
    Changes {
        quotes: Vec<Quote>,
        presentation: ChangePresentation,
    },
}

impl ChunkBlock {
    #[must_use]
    pub fn quotes(&self) -> &[Quote] {
        match self {
            Self::Narrative { .. } => &[],
            Self::Changes { quotes, .. } => quotes,
        }
    }

    pub fn change_ids(&self) -> impl Iterator<Item = &String> {
        self.quotes().iter().map(|quote| &quote.change_id)
    }

    #[must_use]
    pub fn is_narrative(&self) -> bool {
        matches!(self, Self::Narrative { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewChunk {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub kind: ChunkKind,
    pub content: Vec<ChunkBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ChunkContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    /// Why this chunk exceeds the row budget. Rare now that the agent chooses
    /// where a view begins and ends: a view can almost always be made smaller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oversized_reason: Option<String>,
}

impl ReviewChunk {
    pub fn quotes(&self) -> impl Iterator<Item = &Quote> {
        self.content.iter().flat_map(ChunkBlock::quotes)
    }

    pub fn change_ids(&self) -> impl Iterator<Item = &String> {
        self.quotes().map(|quote| &quote.change_id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewSpec {
    pub schema_version: u32,
    /// Identifies the review a human is working through. Stamped once when the
    /// run starts and never derived from spec content, so appending chunks or
    /// fixing a typo cannot orphan their progress.
    #[serde(default)]
    pub review_id: String,
    pub source: SourceIdentity,
    pub overview: Overview,
    pub chunks: Vec<ReviewChunk>,
}

impl ReviewSpec {
    #[must_use]
    pub fn new(source: SourceIdentity, overview: Overview, chunks: Vec<ReviewChunk>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            review_id: String::new(),
            source,
            overview,
            chunks,
        }
    }

    #[must_use]
    pub fn for_review(mut self, review_id: impl Into<String>) -> Self {
        self.review_id = review_id.into();
        self
    }

    /// Where a reviewer's session lives. Falls back to the content hash for
    /// specs written before runs carried an id.
    #[must_use]
    pub fn session_key(&self) -> String {
        if self.review_id.is_empty() {
            self.id()
        } else {
            self.review_id.clone()
        }
    }

    #[must_use]
    pub fn id(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("ReviewSpec serialization cannot fail");
        let digest = hex::encode(Sha256::digest(encoded));
        format!("review-{}", &digest[..16])
    }
}
