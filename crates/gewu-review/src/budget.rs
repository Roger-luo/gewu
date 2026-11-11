//! What a chunk may cost before it must be split or justified.
//!
//! The terminal UI pages, so it holds chunks to rendered rows. The desktop app
//! scrolls, so what costs a reviewer there is attention: it holds chunks to
//! estimated reading time ([`ReadingTime`]), at the rubric's budget.

use std::collections::HashMap;

use crate::{
    CHUNK_HEIGHT_GUIDANCE, Change, ReadingTime, ReviewChunk, ReviewInput, chunk_height,
    load::Rubric,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkBudget {
    /// Rendered rows at the reference terminal width.
    Rows(usize),
    /// Estimated reading seconds.
    Seconds(u32),
}

impl ChunkBudget {
    /// What `chunk` costs in this budget's unit.
    #[must_use]
    pub fn cost(&self, chunk: &ReviewChunk, changes: &HashMap<&String, &Change>) -> f64 {
        match self {
            Self::Rows(_) => chunk_height(chunk, changes) as f64,
            Self::Seconds(_) => ReadingTime::of(chunk, changes).charged(),
        }
    }

    #[must_use]
    pub fn limit(&self) -> f64 {
        match self {
            Self::Rows(rows) => *rows as f64,
            Self::Seconds(seconds) => f64::from(*seconds),
        }
    }

    #[must_use]
    pub fn fits(&self, chunk: &ReviewChunk, changes: &HashMap<&String, &Change>) -> bool {
        self.cost(chunk, changes) <= self.limit()
    }

    /// The budget as a prompt states it: "40 rendered rows", or "about 4 min
    /// of reading as Gewu estimates it".
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Rows(rows) => format!("{rows} rendered rows"),
            Self::Seconds(seconds) => {
                format!(
                    "about {} of reading as Gewu estimates it",
                    clock(f64::from(*seconds))
                )
            }
        }
    }

    /// An amount in this budget's unit, e.g. "52 rows" or "3 min 10 s".
    #[must_use]
    pub fn amount(&self, cost: f64) -> String {
        match self {
            Self::Rows(_) => format!("{} rows", cost.round() as u64),
            Self::Seconds(_) => clock(cost),
        }
    }

    /// How the estimate is made, so an author can budget before measuring.
    #[must_use]
    pub fn guidance(&self) -> String {
        match self {
            Self::Rows(_) => CHUNK_HEIGHT_GUIDANCE.to_owned(),
            Self::Seconds(_) => {
                let rates = &Rubric::builtin().reading;
                format!(
                    "Gewu estimates reading time. Prose costs {:.0} words a minute, a backticked \
                     name counting twice. A changed line of logic costs about {:.0} s, plus {:.0} s \
                     for a branch and {:.0} s for a new name; a removed line {:.0} s; unchanged \
                     context {:.1} s; imports and blank lines almost nothing; a line shaped like \
                     one already shown {:.0} s. Comments cost their words. Explainers cost their \
                     own captions and values, and a stepper over quoted lines makes them cheaper. \
                     A `context` pane is not charged. `gewu harness status` reports each \
                     chunk's estimate; rows are reported too, but do not decide.",
                    rates.words_per_minute,
                    rates.changed_line,
                    rates.branch,
                    rates.new_name,
                    rates.removed_line,
                    rates.context_line,
                    rates.repeated_line,
                )
            }
        }
    }
}

impl ReviewInput {
    /// Reading time when the app set it, otherwise rows.
    #[must_use]
    pub fn chunk_budget(&self) -> ChunkBudget {
        self.max_chunk_seconds.map_or(
            ChunkBudget::Rows(self.max_chunk_lines),
            ChunkBudget::Seconds,
        )
    }
}

fn clock(seconds: f64) -> String {
    let seconds = seconds.round().max(0.0) as u64;
    match (seconds / 60, seconds % 60) {
        (0, s) => format!("{s} s"),
        (m, 0) => format!("{m} min"),
        (m, s) => format!("{m} min {s:02} s"),
    }
}
