//! Whether the scope reached the manifest and every shown line was explained.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{ChunkProse, prose};
use crate::{Change, ChangePresentation, ChunkBlock, Quote, ReviewChunk, ValidatedReview};

#[derive(Clone, Debug, Serialize)]
pub struct CoverageReport {
    /// Scope paths no change in the manifest carries. Empty when no scope was
    /// given, since the manifest is then its own scope.
    pub missing_paths: Vec<String>,
    pub changes: Vec<ChangeCoverage>,
}

/// One change's lines, each counted once under the best treatment any quote
/// gave it.
#[derive(Clone, Debug, Serialize)]
pub struct ChangeCoverage {
    pub change_id: String,
    pub path: String,
    /// Non-blank body lines; blank lines carry nothing to review.
    pub reviewable_lines: usize,
    pub explained_lines: usize,
    /// Behind `summary_only`: described, never shown. Disclosed so a judge can
    /// check each was a generated artifact rather than handwritten behavior.
    pub summarized_lines: usize,
    pub unexplained: Vec<LineRange>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Treatment {
    Unexplained,
    Summarized,
    Explained,
}

impl CoverageReport {
    pub(super) fn new(review: &ValidatedReview, scope: Option<&[String]>) -> Self {
        let known: HashSet<_> = review.input().changes.iter().map(|c| &c.path).collect();
        let missing_paths = scope
            .unwrap_or_default()
            .iter()
            .filter(|path| !known.contains(path))
            .cloned()
            .collect();
        let mut best: HashMap<&str, Vec<Treatment>> = review
            .input()
            .changes
            .iter()
            .map(|c| (c.id.as_str(), vec![Treatment::Unexplained; c.body_lines()]))
            .collect();
        let changes: HashMap<_, _> = review.input().changes.iter().map(|c| (&c.id, c)).collect();
        for chunk in &review.spec().chunks {
            let binding = Binding::new(chunk);
            for block in &chunk.content {
                let ChunkBlock::Changes {
                    quotes,
                    presentation,
                } = block
                else {
                    continue;
                };
                for quote in quotes {
                    let (Some(change), Some(lines)) = (
                        changes.get(&quote.change_id),
                        best.get_mut(quote.change_id.as_str()),
                    ) else {
                        continue;
                    };
                    let treatment = if !binding.explains(change, quote) {
                        Treatment::Unexplained
                    } else if *presentation == ChangePresentation::SummaryOnly {
                        Treatment::Summarized
                    } else {
                        Treatment::Explained
                    };
                    for line in &mut lines[quote.start - 1..quote.end] {
                        *line = (*line).max(treatment);
                    }
                }
            }
        }
        let changes = review
            .input()
            .changes
            .iter()
            .map(|change| ChangeCoverage::new(change, &best[change.id.as_str()]))
            .collect();
        Self {
            missing_paths,
            changes,
        }
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        self.missing_paths.is_empty() && self.changes.iter().all(|c| c.unexplained.is_empty())
    }
}

impl ChangeCoverage {
    fn new(change: &Change, treatments: &[Treatment]) -> Self {
        let reviewable: Vec<_> = (1..=change.body_lines())
            .filter(|line| reviewable(change, *line))
            .collect();
        let count = |wanted| {
            reviewable
                .iter()
                .filter(|line| treatments[*line - 1] == wanted)
                .count()
        };
        let mut unexplained: Vec<LineRange> = Vec::new();
        for &line in &reviewable {
            if treatments[line - 1] != Treatment::Unexplained {
                continue;
            }
            match unexplained.last_mut() {
                Some(range) if range.end + 1 == line => range.end = line,
                _ => unexplained.push(LineRange {
                    start: line,
                    end: line,
                }),
            }
        }
        Self {
            change_id: change.id.clone(),
            path: change.path.clone(),
            reviewable_lines: reviewable.len(),
            explained_lines: count(Treatment::Explained),
            summarized_lines: count(Treatment::Summarized),
            unexplained,
        }
    }
}

/// The prose a chunk offers to explain its quotes.
struct Binding {
    narrated: bool,
    text: String,
    words: HashSet<String>,
    /// Identifiers inside the prose's code spans.
    spanned: HashSet<String>,
}

impl Binding {
    fn new(chunk: &ReviewChunk) -> Self {
        let text = ChunkProse::new(chunk).text();
        let words = prose::identifiers(&text)
            .map(str::to_ascii_lowercase)
            .collect();
        let spanned = prose::code_spans(&text)
            .into_iter()
            .flat_map(prose::identifiers)
            .map(str::to_ascii_lowercase)
            .collect();
        Self {
            // A summary that names the code is narration, and often all a small
            // view needs; demanding a separate block would reward padding.
            narrated: !chunk.summary.trim().is_empty()
                || chunk.content.iter().any(ChunkBlock::is_narrative),
            text,
            words,
            spanned,
        }
    }

    /// A quote is explained when its chunk has prose and that prose names the
    /// quote's file or a distinctive identifier in it. Lines with nothing
    /// distinctive, a lone closing brace say, need only the narration.
    ///
    /// This is necessary, not sufficient: it catches a view nobody wrote
    /// about, never a sentence that is about it but wrong.
    fn explains(&self, change: &Change, quote: &Quote) -> bool {
        if !self.narrated {
            return false;
        }
        let file = change.path.rsplit('/').next().unwrap_or(&change.path);
        if self.text.contains(file) {
            return true;
        }
        let tokens: Vec<_> = change
            .body(quote.start, quote.end)
            .into_iter()
            .flat_map(|text| prose::identifiers(text.get(1..).unwrap_or_default()))
            .collect();
        // A name the prose puts in backticks is an anchor however common the
        // word: `next` named a quote's variable, and the audit ignored it.
        if tokens
            .iter()
            .any(|token| self.spanned.contains(&token.to_ascii_lowercase()))
        {
            return true;
        }
        let mut anchors = tokens
            .into_iter()
            .filter(|token| prose::distinctive(token))
            .peekable();
        anchors.peek().is_none()
            || anchors.any(|token| self.words.contains(&token.to_ascii_lowercase()))
    }
}

/// Mirrors validation, so the audit asks about exactly the lines it requires.
fn reviewable(change: &Change, line: usize) -> bool {
    change.needs_review(line)
}

pub(super) fn shown_lines(review: &ValidatedReview, chunk: &ReviewChunk) -> usize {
    let changes: HashMap<_, _> = review.input().changes.iter().map(|c| (&c.id, c)).collect();
    let mut shown = HashSet::new();
    for quote in chunk.quotes() {
        if let Some(change) = changes.get(&quote.change_id) {
            shown.extend(
                (quote.start..=quote.end)
                    .filter(|line| reviewable(change, *line))
                    .map(|line| (&quote.change_id, line)),
            );
        }
    }
    shown.len()
}
