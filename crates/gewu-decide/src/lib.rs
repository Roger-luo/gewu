//! Typed yes/no decisions about a walkthrough's chunks.
//!
//! A [`Decider`] answers the cognitive-load questions
//! ([`gewu_review::load::CRITERIA`]) about one chunk with probabilities;
//! [`score`] feeds them to the equation in [`gewu_review::load`]. The
//! decision belongs to the equation, not the decider: the decider only says
//! how likely each sign is.
//!
//! Data: [`TypesafeDecider`] sends each chunk's prose and quoted code to
//! TypeSafe (api.typesafe.ai). It runs only when the reviewer opts in.

mod mock;
mod typesafe;

use std::collections::{BTreeMap, HashMap};

use gewu_review::{
    ReadingTime, ReviewInput, ReviewSpec,
    load::{ChunkLoad, Criterion, Rubric, WalkthroughLoad, chunk_state},
};

pub use mock::MockDecider;
pub use typesafe::{DEFAULT_MODEL, DEFAULT_URL, KEY_VARIABLE, TypesafeDecider};

/// Who receives the state text, for the opt-in notice.
pub const PROVIDER: &str = "TypeSafe (api.typesafe.ai)";

/// Answers yes/no questions about a state with probabilities.
pub trait Decider: Send + Sync {
    /// `p(yes)` for each criterion, by name.
    fn answer(
        &self,
        state: &str,
        criteria: &[Criterion],
    ) -> Result<BTreeMap<String, f64>, DecideError>;
}

/// `Display` never contains the API key.
#[derive(Debug, thiserror::Error)]
pub enum DecideError {
    /// Names the environment variable, never its value.
    #[error("{0} is not set")]
    MissingKey(String),
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network: {0}")]
    Network(String),
    #[error("malformed answer: {0}")]
    Malformed(String),
}

/// A walkthrough's load, and the chunks whose questions went unanswered
/// because the decider failed. A failed chunk is still scored on its reading
/// time, so the verdict is a lower bound rather than missing.
#[derive(Debug)]
pub struct Scored {
    pub load: WalkthroughLoad,
    pub failures: Vec<(String, DecideError)>,
}

/// Scores every chunk of `spec` against `rubric`. Without a decider, only
/// reading time counts.
#[must_use]
pub fn score(
    spec: &ReviewSpec,
    input: &ReviewInput,
    rubric: &Rubric,
    decider: Option<&dyn Decider>,
) -> Scored {
    let changes: HashMap<_, _> = input
        .changes
        .iter()
        .map(|change| (&change.id, change))
        .collect();
    let mut failures = Vec::new();
    let chunks = spec
        .chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let reading = ReadingTime::at(chunk, &changes, &rubric.reading);
            let criteria = rubric.questions_for(chunk);
            let answers = match decider {
                None => BTreeMap::new(),
                Some(decider) => decider
                    .answer(
                        &chunk_state(chunk, &changes, &spec.chunks[..index]),
                        &criteria,
                    )
                    .unwrap_or_else(|error| {
                        failures.push((chunk.id.clone(), error));
                        BTreeMap::new()
                    }),
            };
            ChunkLoad::score(&chunk.id, reading, rubric, &criteria, &answers)
        })
        .collect();
    Scored {
        load: WalkthroughLoad::of(chunks, &rubric.equation),
        failures,
    }
}
