//! Whether a walkthrough asks too much of its reader, decided by one equation
//! over per-chunk evidence. Every number in it comes from a [`Rubric`].
//!
//! # The model
//!
//! For each chunk `c`, a decider answers every rubric question `k` that
//! applies to it ([`Rubric::questions_for`]) with a probability `p_ck` that
//! the load sign is present. Each sign is taken as an independent indicator
//! `X_ck ~ Bernoulli(p_ck)`. The chunk's reading time `T_c` ([`ReadingTime`])
//! is measured, not guessed, and enters as a deterministic term. The chunk's
//! load is
//!
//! ```text
//! L_c = Σ_k w_k · X_ck + w_t · max(0, T_c / T_budget − 1)
//! ```
//!
//! and the chunk is overloaded when `L_c ≥ θ`. Because the `X_ck` are
//! independent, `P(L_c ≥ θ)` is exact: the sum over all `2^K` outcomes of
//! their probability, where the outcome crosses `θ`.
//!
//! Over a walkthrough of `N` chunks, the number overloaded follows a
//! Poisson-binomial distribution with parameters `P(L_c ≥ θ)`. The
//! walkthrough has high cognitive overhead when more than a share `α` of its
//! chunks is more likely overloaded than not:
//! `P(#overloaded > ⌊α·N⌋) ≥ 1/2`.
//!
//! An unanswered question counts as absent (`X = 0`), so a score without a
//! decider is a lower bound on load set by reading time alone; the result says
//! how many questions went unanswered.

mod rubric;
mod state;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::ReadingTime;

pub use rubric::{
    Bars, Budget, Criterion, Equation, Guidance, Provenance, Rates, Rubric, RubricError,
};
pub use state::{MAX_STATE_CHARS, chunk_state};

/// Room for float rounding when a sum of weights lands exactly on `θ`.
const EPSILON: f64 = 1e-9;

/// One chunk's evidence and what the equation makes of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkLoad {
    pub id: String,
    pub reading: ReadingTime,
    /// `p_ck` by criterion name, for the criteria a decider answered.
    pub answers: BTreeMap<String, f64>,
    /// `w_t · max(0, T_c / T_budget − 1)`.
    pub time_load: f64,
    /// `E[L_c]`.
    pub expected: f64,
    /// `P(L_c ≥ θ)`.
    pub overloaded: f64,
    /// Criteria with no answer, counted as absent.
    pub unanswered: usize,
    /// What each answered sign, and reading time, adds to `E[L_c]`, largest
    /// first.
    pub drivers: Vec<(String, f64)>,
}

impl ChunkLoad {
    /// Scores one chunk on `questions`, the rubric's questions that apply to
    /// it. Answers outside `[0, 1]` or for other questions are ignored, so a
    /// malformed reply cannot raise or lower the score.
    #[must_use]
    pub fn score(
        id: impl Into<String>,
        reading: ReadingTime,
        rubric: &Rubric,
        questions: &[Criterion],
        answers: &BTreeMap<String, f64>,
    ) -> Self {
        let answered: Vec<(&Criterion, f64)> = questions
            .iter()
            .filter_map(|question| {
                answers
                    .get(&question.name)
                    .copied()
                    .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
                    .map(|p| (question, p))
            })
            .collect();
        let budget = f64::from(rubric.budget.chunk_seconds);
        let equation = rubric.equation;
        let time_load = equation.time_weight * (reading.total() / budget - 1.0).max(0.0);
        let expected = time_load + answered.iter().map(|(q, p)| q.weight * p).sum::<f64>();
        // Every outcome of the answered indicators, as a bit mask.
        let overloaded = (0..1_u32 << answered.len())
            .map(|outcome| {
                let mut probability = 1.0;
                let mut load = time_load;
                for (bit, (question, p)) in answered.iter().enumerate() {
                    if outcome & (1 << bit) == 0 {
                        probability *= 1.0 - p;
                    } else {
                        probability *= p;
                        load += question.weight;
                    }
                }
                if load + EPSILON >= equation.threshold {
                    probability
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            .clamp(0.0, 1.0);
        let mut drivers: Vec<(String, f64)> = answered
            .iter()
            .map(|(question, p)| (question.name.clone(), question.weight * p))
            .chain((time_load > 0.0).then(|| ("reading_time".to_owned(), time_load)))
            .collect();
        drivers.sort_by(|a, b| b.1.total_cmp(&a.1));
        Self {
            id: id.into(),
            reading,
            answers: answered
                .iter()
                .map(|(question, p)| (question.name.clone(), *p))
                .collect(),
            time_load,
            expected,
            overloaded,
            unanswered: questions.len() - answered.len(),
            drivers,
        }
    }
}

/// A walkthrough's chunks and the verdict on the whole.
#[derive(Clone, Debug, PartialEq)]
pub struct WalkthroughLoad {
    pub chunks: Vec<ChunkLoad>,
    /// `P(#overloaded = n)` for `n` in `0..=N`.
    pub distribution: Vec<f64>,
    /// `⌊α·N⌋`: the most chunks that may be overloaded.
    pub allowed: usize,
    /// `P(#overloaded > allowed)`.
    pub excess: f64,
}

impl WalkthroughLoad {
    #[must_use]
    pub fn of(chunks: Vec<ChunkLoad>, equation: &Equation) -> Self {
        // Poisson-binomial by dynamic programming: add one chunk at a time.
        let mut distribution = vec![1.0];
        for chunk in &chunks {
            let mut next = vec![0.0; distribution.len() + 1];
            for (count, probability) in distribution.iter().enumerate() {
                next[count] += probability * (1.0 - chunk.overloaded);
                next[count + 1] += probability * chunk.overloaded;
            }
            distribution = next;
        }
        let allowed = (equation.overloaded_share * chunks.len() as f64).floor() as usize;
        let excess = distribution
            .iter()
            .skip(allowed + 1)
            .sum::<f64>()
            .clamp(0.0, 1.0);
        Self {
            chunks,
            distribution,
            allowed,
            excess,
        }
    }

    /// `E[#overloaded]`.
    #[must_use]
    pub fn expected_overloaded(&self) -> f64 {
        self.chunks.iter().map(|chunk| chunk.overloaded).sum()
    }

    /// The verdict: more than `α` of the chunks more likely overloaded than not.
    #[must_use]
    pub fn high(&self) -> bool {
        self.excess >= 0.5
    }
}
