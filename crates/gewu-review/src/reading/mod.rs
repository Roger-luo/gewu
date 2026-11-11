//! How long a chunk takes to read, as a measure of the work it asks for.
//!
//! Rows say how much a chunk fills the screen; a reviewer in the HTML app
//! scrolls, so what costs them is attention. This estimate prices prose by
//! words, code by what each line asks the reader to follow, and explainers by
//! their own time less what they make easier. It is deterministic and local,
//! and it feeds the cognitive-load score in [`crate::load`].

mod code;
mod explainers;
#[cfg(test)]
mod tests;

use std::{collections::HashMap, fmt};

use crate::{
    Change, ChangePresentation, ChunkBlock, ChunkContext, ReviewChunk,
    load::{Rates, Rubric},
};

use code::{LineCost, Side};

/// Seconds a reviewer spends on one chunk, by what they spend it on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReadingTime {
    /// Title, summary, narrative, and confidence note.
    pub prose: f64,
    /// Quoted source lines and rendered documents.
    pub code: f64,
    /// Explainers' own captions, values, and controls.
    pub explainers: f64,
    /// A `context` pane, read only by a reviewer who needs it.
    pub context: f64,
    /// What steppers save on the quoted lines they walk through.
    pub discount: f64,
}

impl ReadingTime {
    /// At the release rubric's rates.
    #[must_use]
    pub fn of(chunk: &ReviewChunk, changes: &HashMap<&String, &Change>) -> Self {
        Self::at(chunk, changes, &Rubric::builtin().reading)
    }

    /// At `rates`, e.g. another rubric's.
    #[must_use]
    pub fn at(chunk: &ReviewChunk, changes: &HashMap<&String, &Change>, rates: &Rates) -> Self {
        let words = |text: &str| words(text, rates);
        let mut time = Self {
            prose: words(&chunk.title) + words(&chunk.summary),
            context: chunk
                .context
                .as_ref()
                .map_or(0.0, |context| context_words(context, rates)),
            ..Self::default()
        };
        if let Some(confidence) = &chunk.confidence {
            time.prose += words(confidence);
        }
        let mut lines = LineCost::new(rates);
        // Added lines' costs by their trimmed text, so a stepper showing the
        // same code can discount them.
        let mut added: HashMap<String, f64> = HashMap::new();
        for block in &chunk.content {
            match block {
                ChunkBlock::Narrative { markdown } => time.prose += words(markdown),
                ChunkBlock::Explainer(_) => {}
                ChunkBlock::Changes {
                    quotes,
                    presentation,
                } => {
                    for quote in quotes {
                        let Some(change) = changes.get(&quote.change_id) else {
                            continue;
                        };
                        let body = change.body(quote.start, quote.end);
                        time.code += match presentation {
                            ChangePresentation::SummaryOnly => 1.0,
                            ChangePresentation::RenderedMarkdown => {
                                body.iter().map(|line| words(Side::of(line).1)).sum()
                            }
                            ChangePresentation::Diff | ChangePresentation::AddedCode => body
                                .iter()
                                .map(|line| {
                                    let (side, text) = Side::of(line);
                                    let cost = lines.of(side, text);
                                    if side == Side::Added {
                                        added.insert(text.trim().to_owned(), cost);
                                    }
                                    cost
                                })
                                .sum(),
                        };
                    }
                }
            }
        }
        for block in &chunk.content {
            if let ChunkBlock::Explainer(explainer) = block {
                let (own, discount) = explainers::cost(explainer, &added, rates);
                time.explainers += own;
                time.discount += discount;
            }
        }
        time
    }

    /// What the chunk's budget charges: everything but the context pane,
    /// which answers a question the reviewer may not have.
    #[must_use]
    pub fn charged(&self) -> f64 {
        (self.prose + self.code + self.explainers - self.discount).max(0.0)
    }

    /// Everything a reviewer reads, the context pane included.
    #[must_use]
    pub fn total(&self) -> f64 {
        (self.prose + self.code + self.explainers + self.context - self.discount).max(0.0)
    }
}

impl fmt::Display for ReadingTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "about {} to read (prose {}, code {}",
            Clock(self.total()),
            Clock(self.prose),
            Clock(self.code)
        )?;
        if self.explainers > 0.0 {
            write!(f, ", explainers {}", Clock(self.explainers))?;
        }
        if self.context > 0.0 {
            write!(f, ", context pane {}", Clock(self.context))?;
        }
        if self.discount > 0.0 {
            write!(f, ", less {} the steppers save", Clock(self.discount))?;
        }
        f.write_str(")")
    }
}

/// Seconds as "1 min 05 s" or "40 s".
struct Clock(f64);

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let seconds = self.0.round().max(0.0) as u64;
        match seconds / 60 {
            0 => write!(f, "{seconds} s"),
            minutes => write!(f, "{minutes} min {:02} s", seconds % 60),
        }
    }
}

/// Reading seconds for prose. A backticked name is read as two words: it has
/// to be matched against the code, not just recognised.
pub(crate) fn words(text: &str, rates: &Rates) -> f64 {
    let count = text.split_whitespace().count();
    let names = text.matches('`').count() / 2;
    (count + names) as f64 * rates.per_word()
}

fn context_words(context: &ChunkContext, rates: &Rates) -> f64 {
    let words = |text: &str| words(text, rates);
    match context {
        ChunkContext::Concept {
            definition,
            example,
            relation,
        } => words(definition) + words(example) + words(relation),
        ChunkContext::Refactor {
            before,
            after,
            reason,
        } => words(before) + words(after) + words(reason),
    }
}
