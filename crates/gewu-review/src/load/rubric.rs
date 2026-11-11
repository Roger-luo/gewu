//! The cognitive-load rubric as data: questions, weights, the equation's
//! settings, reading rates, the chunk budget, and the guidance for changing
//! them. The release's copy is `rubrics/load.toml`, built into the binary;
//! any other copy can be loaded in its place.

use std::{collections::HashSet, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::{ChunkKind, ReviewChunk};

const BUILTIN: &str = include_str!("../../rubrics/load.toml");

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rubric {
    /// Bumped with every change; scores from different versions are never
    /// compared.
    pub version: u32,
    pub summary: String,
    pub provenance: Provenance,
    pub guidance: Guidance,
    pub budget: Budget,
    pub equation: Equation,
    pub reading: Rates,
    pub bars: Bars,
    #[serde(rename = "question")]
    pub questions: Vec<Criterion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub calibrated: String,
    pub data: String,
    pub results: String,
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Guidance {
    pub overloaded: String,
    pub labelling: String,
    pub recalibrate: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Reading seconds a chunk may take in the app before it must be split
    /// or justified.
    pub chunk_seconds: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Equation {
    /// `θ`: the load at which a chunk counts as overloaded.
    pub threshold: f64,
    /// `w_t`: load per budget's worth of reading past the first.
    pub time_weight: f64,
    /// `α`: the share of chunks that may be overloaded before the
    /// walkthrough is.
    pub overloaded_share: f64,
}

/// Seconds per thing read; see `rubrics/load.toml` for each.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    pub words_per_minute: f64,
    pub changed_line: f64,
    pub removed_line: f64,
    pub context_line: f64,
    pub import_line: f64,
    pub trivial_line: f64,
    pub repeated_line: f64,
    pub branch: f64,
    pub new_name: f64,
    pub nesting_level: f64,
    pub line_ceiling: f64,
    pub figure: f64,
    pub stepper_step: f64,
    pub value: f64,
    pub highlighted_line: f64,
    pub unquoted_stepper_line: f64,
    pub cost_model_param: f64,
    /// The share of a quoted line's cost a stepper over it saves.
    pub stepper_discount: f64,
}

impl Rates {
    #[must_use]
    pub fn per_word(&self) -> f64 {
        60.0 / self.words_per_minute
    }
}

/// What a calibration must reach before the score may gate anything.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bars {
    pub question_auc: f64,
    pub min_labels: u32,
    pub model_auc: f64,
    pub accuracy: f64,
    pub brier: f64,
    pub jev_margin: f64,
    pub max_spend_usd: f64,
    /// The most of the chunks labelled fine a proposed budget may flag.
    pub max_false_flags: f64,
}

/// One yes/no question, with the weight its "yes" adds to a chunk's load.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    /// The answer's key, stable across versions of the wording.
    pub name: String,
    pub instructions: String,
    pub when_true: String,
    pub when_false: String,
    /// Load added by a "yes", in units of the equation's threshold.
    pub weight: f64,
    /// Asked of test chunks too.
    pub tests: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RubricError {
    #[error("not a rubric: {0}")]
    Parse(String),
    #[error("invalid rubric: {0}")]
    Invalid(String),
}

impl Rubric {
    /// The release's rubric. It is checked by a test, so this cannot fail in
    /// a released binary.
    #[must_use]
    pub fn builtin() -> &'static Self {
        static RUBRIC: OnceLock<Rubric> = OnceLock::new();
        RUBRIC.get_or_init(|| Self::parse(BUILTIN).expect("the built-in rubric is valid"))
    }

    /// Parses and checks a rubric file's text.
    pub fn parse(text: &str) -> Result<Self, RubricError> {
        let rubric: Self = toml::from_str(text).map_err(|e| RubricError::Parse(e.to_string()))?;
        rubric.check().map_err(RubricError::Invalid)?;
        Ok(rubric)
    }

    /// The questions that apply to `chunk`: test chunks skip those about prose.
    #[must_use]
    pub fn questions_for(&self, chunk: &ReviewChunk) -> Vec<Criterion> {
        let test = chunk.kind == ChunkKind::Test;
        self.questions
            .iter()
            .filter(|question| question.tests || !test)
            .cloned()
            .collect()
    }

    fn check(&self) -> Result<(), String> {
        let positive = |name: &str, value: f64| {
            (value.is_finite() && value > 0.0)
                .then_some(())
                .ok_or(format!("`{name}` must be a positive number, not {value}"))
        };
        if self.version == 0 {
            return Err("`version` must be at least 1".into());
        }
        if self.budget.chunk_seconds == 0 {
            return Err("`budget.chunk_seconds` must be at least 1".into());
        }
        positive("equation.threshold", self.equation.threshold)?;
        positive("reading.words_per_minute", self.reading.words_per_minute)?;
        if !(0.0..=1.0).contains(&self.equation.overloaded_share) {
            return Err("`equation.overloaded_share` must be between 0 and 1".into());
        }
        if !(0.0..=1.0).contains(&self.reading.stepper_discount) {
            return Err("`reading.stepper_discount` must be between 0 and 1".into());
        }
        if self.questions.len() > 16 {
            return Err("at most 16 questions: the score enumerates every outcome".into());
        }
        let mut names = HashSet::new();
        for question in &self.questions {
            if !names.insert(question.name.as_str()) {
                return Err(format!("question `{}` appears twice", question.name));
            }
            positive(
                &format!("question `{}` weight", question.name),
                question.weight,
            )?;
            if question.instructions.trim().is_empty() {
                return Err(format!("question `{}` has no instructions", question.name));
            }
        }
        Ok(())
    }
}
