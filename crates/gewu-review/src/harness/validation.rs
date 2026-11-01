use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{
    Change, ChangePresentation, ChunkBlock, Quote, ReviewChunk, ReviewInput, chunk_height,
};

use super::{
    Expansion, HARNESS_SCHEMA_VERSION, InvestigationCheckpoint, Revision, WorkItem, WorkList,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HarnessValidationError {
    #[error("unsupported checkpoint schema version {0}")]
    Schema(u32),
    #[error("checkpoint belongs to a different review source")]
    StaleSource,
    #[error("investigation intent must not be empty")]
    EmptyIntent,
    #[error("checkpoint contains no {0}")]
    Empty(&'static str),
    #[error("an overview may carry at most five change bullets")]
    TooManyOverviewChanges,
    #[error("unknown change id `{0}`")]
    UnknownChange(String),
    #[error("change id `{0}` has not been investigated")]
    UninvestigatedChange(String),
    #[error("nothing shows line {line} of change `{change}`")]
    UncoveredLines { change: String, line: usize },
    #[error("quote of `{change}` names lines {start}-{end}, but it has {lines}")]
    UnreadableQuote {
        change: String,
        start: usize,
        end: usize,
        lines: usize,
    },
    #[error("duplicate chunk id `{0}`")]
    DuplicateChunk(String),
    #[error("chunk `{0}` has an empty title or no content")]
    EmptyChunk(String),
    #[error(
        "chunk `{chunk}` needs about {lines} rows but the budget is {budget}; split it, or explain it in `oversized_reason`"
    )]
    OversizedChunk {
        chunk: String,
        lines: usize,
        budget: usize,
    },
    #[error("duplicate work item id `{0}`")]
    DuplicateItem(String),
    #[error("work item `{0}` has an empty title or claims no changes")]
    EmptyItem(String),
    #[error(
        "work item `{item}` owns {lines} visible source rows but its {chunks}-chunk budget can show at most {budget}; split the item, or isolate an intentionally summarized generated artifact in its own item"
    )]
    ImpossibleItem {
        item: String,
        lines: usize,
        chunks: usize,
        budget: usize,
    },
    #[error("item `{item}` expanded into {chunks} chunks but the cap is {cap}")]
    TooManyChunks {
        item: String,
        chunks: usize,
        cap: usize,
    },
    #[error("chunk quotes lines of `{change}` that item `{item}` does not cover")]
    QuoteOutsideItem { item: String, change: String },
    #[error("chunk `{0}` was asked to be rewritten but the revision does not replace it")]
    MissingRewrite(String),
    #[error("chunk `{0}` was not asked to be rewritten")]
    UnexpectedRewrite(String),
    #[error("rewritten chunk `{chunk}` quotes lines of `{change}` the original did not show")]
    QuoteOutsideRewrite { chunk: String, change: String },
    #[error("chunk `{0}` requests rendered Markdown for a non-Markdown change")]
    InvalidMarkdownPresentation(String),
}

#[derive(Debug, Error)]
#[error("harness checkpoint validation failed")]
pub struct HarnessValidationErrors(pub Vec<HarnessValidationError>);

pub(super) struct CheckpointValidator;

impl CheckpointValidator {
    pub(super) fn investigation(
        checkpoint: &InvestigationCheckpoint,
        input: &ReviewInput,
    ) -> Result<(), HarnessValidationErrors> {
        let mut errors = Self::header(checkpoint.schema_version, &checkpoint.source, input);
        if checkpoint.intent.trim().is_empty() {
            errors.push(HarnessValidationError::EmptyIntent);
        }
        if checkpoint.findings.is_empty() {
            errors.push(HarnessValidationError::Empty("findings"));
        }
        if checkpoint.changes.is_empty() {
            errors.push(HarnessValidationError::Empty("overview changes"));
        }
        if checkpoint.changes.len() > 5 {
            errors.push(HarnessValidationError::TooManyOverviewChanges);
        }
        let known: HashSet<_> = input.changes.iter().map(|change| &change.id).collect();
        let mut seen = HashSet::new();
        for finding in &checkpoint.findings {
            for id in &finding.change_ids {
                if !known.contains(id) {
                    errors.push(HarnessValidationError::UnknownChange(id.clone()));
                }
                seen.insert(id);
            }
        }
        for change in &input.changes {
            if !seen.contains(&change.id) {
                errors.push(HarnessValidationError::UninvestigatedChange(
                    change.id.clone(),
                ));
            }
        }
        Self::result(errors)
    }

    /// A work list must account for the whole manifest. Items may overlap —
    /// the same lines can belong to two parts of the story — so what matters is
    /// that their spans, taken together, leave nothing unreviewed.
    pub(super) fn work_list(
        list: &WorkList,
        input: &ReviewInput,
    ) -> Result<(), HarnessValidationErrors> {
        Self::work_list_with_budget(list, input, None)
    }

    pub(super) fn work_list_for(
        list: &WorkList,
        input: &ReviewInput,
        max_chunks: usize,
    ) -> Result<(), HarnessValidationErrors> {
        Self::work_list_with_budget(list, input, Some(max_chunks))
    }

    fn work_list_with_budget(
        list: &WorkList,
        input: &ReviewInput,
        max_chunks: Option<usize>,
    ) -> Result<(), HarnessValidationErrors> {
        let mut errors = Self::header(list.schema_version, &list.source, input);
        if list.items.is_empty() && !input.changes.is_empty() {
            errors.push(HarnessValidationError::Empty("work items"));
        }
        let mut item_ids = HashSet::new();
        for item in &list.items {
            if !item_ids.insert(&item.id) {
                errors.push(HarnessValidationError::DuplicateItem(item.id.clone()));
            }
            if item.title.trim().is_empty() || item.covers.is_empty() {
                errors.push(HarnessValidationError::EmptyItem(item.id.clone()));
            }
            errors.extend(Self::resolvable(&item.covers, input));
            if let Some(max_chunks) = max_chunks {
                errors.extend(Self::item_budget(item, input, max_chunks));
            }
        }
        let covers: Vec<_> = list
            .items
            .iter()
            .flat_map(|item| item.covers.iter())
            .collect();
        errors.extend(Self::uncovered(&covers, input.changes.iter().collect()));
        Self::result(errors)
    }

    /// Rejects broad work items whose ordinary source cannot fit the entire
    /// expansion allowance. A one-change item is exempt because generated
    /// artifacts deliberately support `summary_only`; isolating such an
    /// artifact is also the remedy suggested to authors.
    fn item_budget(
        item: &WorkItem,
        input: &ReviewInput,
        max_chunks: usize,
    ) -> Vec<HarnessValidationError> {
        let change_ids: HashSet<_> = item.change_ids().collect();
        if change_ids.len() <= 1 {
            return Vec::new();
        }

        let known: HashMap<_, _> = input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        let visible_rows: HashSet<_> = item
            .covers
            .iter()
            .filter_map(|quote| known.get(&quote.change_id).map(|change| (quote, *change)))
            .flat_map(|(quote, change)| {
                (quote.start..=quote.end.min(change.body_lines()))
                    .filter(|line| !Self::is_blank(change, *line))
                    .map(move |line| (&quote.change_id, line))
            })
            .collect();
        let budget = input.max_chunk_lines.saturating_mul(max_chunks);
        if visible_rows.len() <= budget {
            Vec::new()
        } else {
            vec![HarnessValidationError::ImpossibleItem {
                item: item.id.clone(),
                lines: visible_rows.len(),
                chunks: max_chunks,
                budget,
            }]
        }
    }

    /// An expansion walks through exactly its item's spans, in as many views as
    /// the narrative needs, and no more chunks than the reviewer agreed to face.
    pub(super) fn expansion(
        expansion: &Expansion,
        input: &ReviewInput,
        item: &WorkItem,
        max_chunks: usize,
    ) -> Result<(), HarnessValidationErrors> {
        let mut errors = Self::header(expansion.schema_version, &expansion.source, input);
        if expansion.chunks.is_empty() {
            errors.push(HarnessValidationError::Empty("chunks"));
        }
        if expansion.chunks.len() > max_chunks {
            errors.push(HarnessValidationError::TooManyChunks {
                item: item.id.clone(),
                chunks: expansion.chunks.len(),
                cap: max_chunks,
            });
        }
        errors.extend(Self::chunks(&expansion.chunks, input));
        errors.extend(Self::within(&expansion.chunks, &item.covers, |change| {
            HarnessValidationError::QuoteOutsideItem {
                item: item.id.clone(),
                change,
            }
        }));
        // The item's own spans must all be walked through by its chunks.
        let quoted: Vec<_> = expansion
            .chunks
            .iter()
            .flat_map(ReviewChunk::quotes)
            .collect();
        errors.extend(Self::unwalked(&quoted, &item.covers, input));
        Self::result(errors)
    }

    /// A rewrite answers feedback about how chunks read, so it replaces exactly
    /// the chunks it was asked for and shows exactly the lines they showed.
    /// Everything above that — the prose, the order of views, where one view
    /// ends and the next begins — is what the reviewer asked to be changed.
    pub(super) fn revision(
        revision: &Revision,
        input: &ReviewInput,
        replacing: &[&ReviewChunk],
    ) -> Result<(), HarnessValidationErrors> {
        let mut errors = Self::header(revision.schema_version, &revision.source, input);
        let rewritten: HashSet<_> = revision.chunks.iter().map(|chunk| &chunk.id).collect();
        let wanted: HashSet<_> = replacing.iter().map(|chunk| &chunk.id).collect();
        for id in wanted.difference(&rewritten) {
            errors.push(HarnessValidationError::MissingRewrite((*id).clone()));
        }
        for id in rewritten.difference(&wanted) {
            errors.push(HarnessValidationError::UnexpectedRewrite((*id).clone()));
        }
        errors.extend(Self::chunks(&revision.chunks, input));
        // Each rewrite is held to the lines its original showed: no more, so a
        // rewrite cannot annex a neighbour's code, and no fewer, so the spec
        // still accounts for every line the review had covered.
        for chunk in &revision.chunks {
            let Some(original) = replacing.iter().find(|other| other.id == chunk.id) else {
                continue;
            };
            let covers: Vec<_> = original.quotes().cloned().collect();
            errors.extend(Self::within(
                std::slice::from_ref(chunk),
                &covers,
                |change| HarnessValidationError::QuoteOutsideRewrite {
                    chunk: chunk.id.clone(),
                    change,
                },
            ));
            let quoted: Vec<_> = chunk.quotes().collect();
            errors.extend(Self::unwalked(&quoted, &covers, input));
        }
        Self::result(errors)
    }

    /// The checks every chunk faces, wherever it came from.
    fn chunks(chunks: &[ReviewChunk], input: &ReviewInput) -> Vec<HarnessValidationError> {
        let known: HashMap<_, _> = input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        let mut errors = Vec::new();
        let mut chunk_ids = HashSet::new();
        for chunk in chunks {
            if !chunk_ids.insert(&chunk.id) {
                errors.push(HarnessValidationError::DuplicateChunk(chunk.id.clone()));
            }
            let empty = chunk.title.trim().is_empty()
                || chunk.content.is_empty()
                || chunk.content.iter().all(|block| match block {
                    ChunkBlock::Narrative { markdown } => markdown.trim().is_empty(),
                    ChunkBlock::Changes { quotes, .. } => quotes.is_empty(),
                });
            if empty {
                errors.push(HarnessValidationError::EmptyChunk(chunk.id.clone()));
            }
            errors.extend(Self::resolvable(
                &chunk.quotes().cloned().collect::<Vec<_>>(),
                input,
            ));
            let invalid_markdown = chunk.content.iter().any(|block| match block {
                ChunkBlock::Changes {
                    quotes,
                    presentation: ChangePresentation::RenderedMarkdown,
                } => quotes.iter().any(|quote| {
                    known
                        .get(&quote.change_id)
                        .is_some_and(|change| !change.path.ends_with(".md"))
                }),
                _ => false,
            });
            if invalid_markdown {
                errors.push(HarnessValidationError::InvalidMarkdownPresentation(
                    chunk.id.clone(),
                ));
            }
            let lines = chunk_height(chunk, &known);
            if lines > input.max_chunk_lines
                && chunk.oversized_reason.as_deref().is_none_or(str::is_empty)
            {
                errors.push(HarnessValidationError::OversizedChunk {
                    chunk: chunk.id.clone(),
                    lines,
                    budget: input.max_chunk_lines,
                });
            }
        }
        errors
    }

    /// Quotes that reach outside the spans they are allowed to show. Measured
    /// against the union of those spans, so a view may bridge two of them where
    /// the story needs it.
    fn within(
        chunks: &[ReviewChunk],
        covers: &[Quote],
        error: impl Fn(String) -> HarnessValidationError,
    ) -> Vec<HarnessValidationError> {
        chunks
            .iter()
            .flat_map(ReviewChunk::quotes)
            .filter(|quote| {
                (quote.start..=quote.end).any(|line| {
                    !covers
                        .iter()
                        .any(|owned| owned.change_id == quote.change_id && owned.covers(line))
                })
            })
            .map(|quote| error(quote.change_id.clone()))
            .collect()
    }

    /// Reports quotes that name a change the manifest does not hold, or a span
    /// that runs past the end of one.
    fn resolvable(quotes: &[Quote], input: &ReviewInput) -> Vec<HarnessValidationError> {
        let known: HashMap<_, _> = input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        quotes
            .iter()
            .filter_map(|quote| match known.get(&quote.change_id) {
                None => Some(HarnessValidationError::UnknownChange(
                    quote.change_id.clone(),
                )),
                Some(change) => {
                    (quote.start == 0 || quote.end < quote.start || quote.end > change.body_lines())
                        .then(|| HarnessValidationError::UnreadableQuote {
                            change: quote.change_id.clone(),
                            start: quote.start,
                            end: quote.end,
                            lines: change.body_lines(),
                        })
                }
            })
            .collect()
    }

    /// Lines of the manifest no span reaches. Blank lines are exempt: they
    /// carry nothing to review, and the renderer trims them from a view's edge
    /// anyway, so demanding them would force a span to claim a row that is
    /// never drawn.
    fn uncovered(quotes: &[&Quote], changes: Vec<&Change>) -> Vec<HarnessValidationError> {
        changes
            .into_iter()
            .filter_map(|change| {
                let missing = (1..=change.body_lines())
                    .filter(|line| !Self::is_blank(change, *line))
                    .find(|line| {
                        !quotes
                            .iter()
                            .any(|quote| quote.change_id == change.id && quote.covers(*line))
                    });
                missing.map(|line| HarnessValidationError::UncoveredLines {
                    change: change.id.clone(),
                    line,
                })
            })
            .collect()
    }

    fn is_blank(change: &Change, line: usize) -> bool {
        !change.needs_review(line)
    }

    /// Spans of the item that no chunk walks through.
    fn unwalked(
        quoted: &[&Quote],
        covers: &[Quote],
        input: &ReviewInput,
    ) -> Vec<HarnessValidationError> {
        let known: HashMap<_, _> = input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        covers
            .iter()
            .filter_map(|owned| {
                let limit = known
                    .get(&owned.change_id)
                    .map_or(owned.end, |change| owned.end.min(change.body_lines()));
                let change = known.get(&owned.change_id);
                let missing = (owned.start..=limit)
                    .filter(|line| change.is_none_or(|change| !Self::is_blank(change, *line)))
                    .find(|line| {
                        !quoted
                            .iter()
                            .any(|quote| quote.change_id == owned.change_id && quote.covers(*line))
                    });
                missing.map(|line| HarnessValidationError::UncoveredLines {
                    change: owned.change_id.clone(),
                    line,
                })
            })
            .collect()
    }

    fn header(
        version: u32,
        source: &crate::SourceIdentity,
        input: &ReviewInput,
    ) -> Vec<HarnessValidationError> {
        let mut errors = Vec::new();
        if version != HARNESS_SCHEMA_VERSION {
            errors.push(HarnessValidationError::Schema(version));
        }
        if source != &input.source {
            errors.push(HarnessValidationError::StaleSource);
        }
        errors
    }

    fn result(errors: Vec<HarnessValidationError>) -> Result<(), HarnessValidationErrors> {
        if errors.is_empty() {
            Ok(())
        } else {
            Err(HarnessValidationErrors(errors))
        }
    }
}
