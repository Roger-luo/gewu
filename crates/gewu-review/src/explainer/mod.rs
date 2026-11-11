//! Explainers: small, typed teaching blocks a walkthrough step can carry
//! beside its prose and quotes. The agent fills in data; Gewu draws it, so
//! every explainer looks the same, renders safely, and can be checked.
//!
//! - [`Explainer::Claim`] ties a step to something the change claims, quoted
//!   from the author's description, and says how this step serves it.
//! - [`Explainer::Layout`] draws records as boxes scaled to their bytes, so a
//!   reviewer sees what each entry holds before and after.
//! - [`Explainer::CostModel`] computes costs from parameters the reviewer
//!   drags, with [`formula`] arithmetic, so the claim can be checked.

pub mod formula;
#[cfg(test)]
mod tests;
mod text;
mod validate;

pub use validate::unquoted_claims;

pub use text::format_value;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Explainer {
    Claim {
        /// Verbatim from the author's description, e.g. a benchmark claim.
        claim: String,
        /// The measured result the claim reports, e.g. "300 → 105 MiB (−65%)".
        /// It belongs to the whole change, so it needs `share`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        /// This step's part of `result`, e.g. "about 36 MiB of the 573 MiB
        /// drop (≈6%)", so a part is never credited with the whole.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        share: Option<String>,
        /// Markdown: how this step contributes to the claim.
        how: String,
    },
    Layout {
        title: String,
        /// Usually two: before, then after.
        rows: Vec<LayoutRow>,
        /// Where the sizes come from: a type, a `size_of` test, a comment.
        source: String,
    },
    CostModel {
        title: String,
        params: Vec<Param>,
        series: Vec<Series>,
        /// What the model leaves out, so the reviewer reads it as an estimate.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        source: String,
    },
    /// An algorithm walked one state at a time: the code, the lines that
    /// run now, and the values that changed.
    Stepper {
        title: String,
        /// Shiki's language name, e.g. `rust`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        /// Where the code comes from, e.g. `src/compact_index.rs:37-41`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// The file line number of the code's first line, so the gutter and
        /// `highlight` use the same numbers as the diff. Defaults to 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start_line: Option<usize>,
        steps: Vec<Step>,
    },
    /// Work done once versus in every step, as lanes over a row of steps.
    Timeline {
        title: String,
        /// How many steps to draw, e.g. 6.
        steps: u32,
        /// What one step is called, e.g. "pc_step".
        step_label: String,
        lanes: Vec<Lane>,
        /// The unit `cost` counts, e.g. "allocations" or "ms".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Step {
    /// Markdown: what happens in this step.
    pub caption: String,
    /// The code at this step; omitted keeps the previous step's code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Lines that run now, numbered from the stepper's `start_line`:
    /// "38", "38-40", "37,40-41".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<StateValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateValue {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Lane {
    /// Usually "Before" and "After".
    pub label: String,
    pub work: Vec<Work>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Work {
    pub label: String,
    pub when: When,
    /// What it costs each time it runs, in the timeline's `unit`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum When {
    /// In the first step only, then reused.
    Once,
    /// In every step.
    Each,
}

/// Parses "3", "3-5", or "2,6-7" into inclusive 1-based line ranges.
pub fn line_ranges(text: &str) -> Result<Vec<(usize, usize)>, String> {
    text.split(',')
        .map(|part| {
            let part = part.trim();
            let (start, end) = part.split_once('-').unwrap_or((part, part));
            let parse = |n: &str| {
                n.trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| format!("`{text}` is not a list of lines like 3, 3-5, or 2,6-7"))
            };
            let (start, end) = (parse(start)?, parse(end)?);
            if start > end {
                return Err(format!("`{part}` runs backwards"));
            }
            Ok((start, end))
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayoutRow {
    pub label: String,
    pub fields: Vec<LayoutField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LayoutField {
    pub name: String,
    pub bytes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A whole-number parameter the reviewer drags. Fractions belong in formulas.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Param {
    /// Named in formulas: lowercase letters, digits, and `_`.
    pub id: String,
    pub label: String,
    pub min: i64,
    pub max: i64,
    pub default: i64,
    #[serde(default)]
    pub scale: Scale,
    /// Values worth landing on, such as the sizes a benchmark measured;
    /// drawn as labeled stops under the slider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<i64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    #[default]
    Linear,
    /// Steps through powers of two between `min` and `max`.
    Log2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Series {
    pub label: String,
    pub formula: String,
    #[serde(default)]
    pub unit: Unit,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Bytes,
    #[default]
    Count,
    Seconds,
}

impl Explainer {
    /// Reader-facing prose the audits read with the narrative.
    #[must_use]
    pub fn prose(&self) -> Vec<&str> {
        match self {
            Self::Claim { how, .. } => vec![how],
            Self::CostModel { note, .. } => note.as_deref().into_iter().collect(),
            Self::Stepper { steps, .. } => steps.iter().map(|step| step.caption.as_str()).collect(),
            Self::Layout { .. } | Self::Timeline { .. } => Vec::new(),
        }
    }

    /// About how many terminal rows it takes beside its prose, for the
    /// chunk height budget.
    #[must_use]
    pub fn rows(&self) -> usize {
        match self {
            Self::Claim { result, share, .. } => {
                3 + usize::from(result.is_some()) + usize::from(share.is_some())
            }
            Self::Layout { rows, .. } => 2 + rows.len() * 2,
            Self::CostModel { params, series, .. } => 3 + params.len() * 2 + series.len(),
            Self::Stepper { steps, .. } => {
                let longest = steps
                    .iter()
                    .filter_map(|step| step.code.as_deref())
                    .map(|code| code.lines().count())
                    .max()
                    .unwrap_or(0);
                4 + longest
            }
            Self::Timeline { lanes, .. } => 3 + lanes.len() * 2,
        }
    }
}
