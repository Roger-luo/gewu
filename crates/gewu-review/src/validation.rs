use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{ChunkBlock, ReviewInput, ReviewSpec, SCHEMA_VERSION};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValidationError {
    #[error("unsupported {artifact} schema version {actual}; expected {expected}")]
    Schema {
        artifact: &'static str,
        actual: u32,
        expected: u32,
    },
    #[error("review spec was generated for a different source")]
    StaleSource,
    #[error("overview intent must not be empty")]
    EmptyIntent,
    #[error("overview must contain at most five change bullets")]
    TooManyOverviewChanges,
    #[error("chunk `{0}` has an empty title or summary")]
    EmptyChunkText(String),
    #[error("chunk `{0}` summary exceeds 500 characters")]
    SummaryTooLong(String),
    #[error("chunk `{0}` has empty review content")]
    EmptyChunkContent(String),
    #[error("duplicate chunk id `{0}`")]
    DuplicateChunk(String),
    #[error("unknown change id `{0}`")]
    UnknownChange(String),
    #[error("nothing shows line {line} of change `{change}`")]
    UncoveredLines { change: String, line: usize },
    #[error("quote of `{change}` names lines {start}-{end}, but it has {lines}")]
    UnreadableQuote {
        change: String,
        start: usize,
        end: usize,
        lines: usize,
    },
}

#[derive(Debug, Error)]
#[error("review spec validation failed")]
pub struct ValidationErrors(pub Vec<ValidationError>);

#[derive(Clone, Debug)]
pub struct ValidatedReview {
    input: ReviewInput,
    spec: ReviewSpec,
}

/// Whether the spec is expected to account for the whole manifest yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Coverage {
    /// Every change must appear; the review is finished being generated.
    Complete,
    /// Changes may still be unexpanded, but nothing may be quoted twice or
    /// invented.
    Partial,
}

impl ValidatedReview {
    pub fn validate(input: ReviewInput, spec: ReviewSpec) -> Result<Self, ValidationErrors> {
        Self::checked(input, spec, Coverage::Complete)
    }

    /// Validates a review that is still being expanded one work item at a time.
    /// A reviewer opens these mid-generation, so unexpanded changes are not an
    /// error; the work list is what promises they will be covered.
    pub fn validate_partial(
        input: ReviewInput,
        spec: ReviewSpec,
    ) -> Result<Self, ValidationErrors> {
        Self::checked(input, spec, Coverage::Partial)
    }

    fn checked(
        input: ReviewInput,
        spec: ReviewSpec,
        coverage: Coverage,
    ) -> Result<Self, ValidationErrors> {
        let errors = Validator::new(&input, &spec, coverage).validate();
        if errors.is_empty() {
            Ok(Self { input, spec })
        } else {
            Err(ValidationErrors(errors))
        }
    }

    #[must_use]
    pub fn input(&self) -> &ReviewInput {
        &self.input
    }

    #[must_use]
    pub fn spec(&self) -> &ReviewSpec {
        &self.spec
    }
}

struct Validator<'a> {
    input: &'a ReviewInput,
    spec: &'a ReviewSpec,
    coverage: Coverage,
    errors: Vec<ValidationError>,
}

impl<'a> Validator<'a> {
    fn new(input: &'a ReviewInput, spec: &'a ReviewSpec, coverage: Coverage) -> Self {
        Self {
            input,
            spec,
            coverage,
            errors: Vec::new(),
        }
    }

    fn validate(mut self) -> Vec<ValidationError> {
        self.validate_header();
        self.validate_content();
        self.validate_coverage();
        self.errors
    }

    fn validate_header(&mut self) {
        for (artifact, version) in [
            ("input", self.input.schema_version),
            ("spec", self.spec.schema_version),
        ] {
            if version != SCHEMA_VERSION {
                self.errors.push(ValidationError::Schema {
                    artifact,
                    actual: version,
                    expected: SCHEMA_VERSION,
                });
            }
        }
        if self.input.source != self.spec.source {
            self.errors.push(ValidationError::StaleSource);
        }
    }

    fn validate_content(&mut self) {
        if self.spec.overview.intent.trim().is_empty() {
            self.errors.push(ValidationError::EmptyIntent);
        }
        if self.spec.overview.changes.len() > 5 {
            self.errors.push(ValidationError::TooManyOverviewChanges);
        }
        let mut ids = HashSet::new();
        for chunk in &self.spec.chunks {
            if !ids.insert(&chunk.id) {
                self.errors
                    .push(ValidationError::DuplicateChunk(chunk.id.clone()));
            }
            if chunk.title.trim().is_empty() || chunk.summary.trim().is_empty() {
                self.errors
                    .push(ValidationError::EmptyChunkText(chunk.id.clone()));
            }
            if chunk.summary.chars().count() > 500 {
                self.errors
                    .push(ValidationError::SummaryTooLong(chunk.id.clone()));
            }
            let empty = chunk.content.is_empty()
                || chunk.content.iter().all(|block| match block {
                    ChunkBlock::Narrative { markdown } => markdown.trim().is_empty(),
                    ChunkBlock::Changes { quotes, .. } => quotes.is_empty(),
                });
            if empty {
                self.errors
                    .push(ValidationError::EmptyChunkContent(chunk.id.clone()));
            }
        }
    }

    /// A quote may name any span of any change, as often as the walkthrough
    /// needs it. What a finished review must satisfy is that the spans, taken
    /// together, leave no line unshown.
    fn validate_coverage(&mut self) {
        let known: HashMap<_, _> = self
            .input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        let quotes: Vec<_> = self
            .spec
            .chunks
            .iter()
            .flat_map(crate::ReviewChunk::quotes)
            .collect();
        for quote in &quotes {
            match known.get(&quote.change_id) {
                None => self
                    .errors
                    .push(ValidationError::UnknownChange(quote.change_id.clone())),
                Some(change) => {
                    if quote.start == 0
                        || quote.end < quote.start
                        || quote.end > change.body_lines()
                    {
                        self.errors.push(ValidationError::UnreadableQuote {
                            change: quote.change_id.clone(),
                            start: quote.start,
                            end: quote.end,
                            lines: change.body_lines(),
                        });
                    }
                }
            }
        }
        if self.coverage != Coverage::Complete {
            return;
        }
        for change in &self.input.changes {
            // Blank lines and edge context carry nothing to review, so they
            // are exempt here exactly as they are in the harness validator.
            if let Some(line) = (1..=change.body_lines())
                .filter(|line| change.needs_review(*line))
                .find(|line| {
                    !quotes
                        .iter()
                        .any(|quote| quote.change_id == change.id && quote.covers(*line))
                })
            {
                self.errors.push(ValidationError::UncoveredLines {
                    change: change.id.clone(),
                    line,
                });
            }
        }
    }
}
