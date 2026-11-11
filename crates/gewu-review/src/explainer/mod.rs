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

/// Claims in `chunks` that the author's `description` does not contain word
/// for word (whitespace aside), as errors an agent can act on. With no
/// description, every claim is refused: there is nothing to quote.
#[must_use]
pub fn unquoted_claims<'a>(
    chunks: impl IntoIterator<Item = &'a crate::ReviewChunk>,
    description: Option<&str>,
) -> Vec<String> {
    let squash = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let haystack = description.map(squash);
    let mut errors = Vec::new();
    for chunk in chunks {
        for block in &chunk.content {
            let crate::ChunkBlock::Explainer(Explainer::Claim { claim, .. }) = block else {
                continue;
            };
            match &haystack {
                None => errors.push(format!(
                    "chunk `{}` quotes a claim, but this review has no author description to \
                     quote; say what the step achieves in the narrative instead",
                    chunk.id
                )),
                Some(text) if !text.contains(&squash(claim)) => errors.push(format!(
                    "chunk `{}` quotes a claim that is not in the author's description word for \
                     word: \"{claim}\"; copy the sentence from description.md",
                    chunk.id
                )),
                Some(_) => {}
            }
        }
    }
    errors
}

const MAX_ROWS: usize = 4;
const MAX_FIELDS: usize = 8;
const MAX_PARAMS: usize = 3;
const MAX_SERIES: usize = 4;

impl Explainer {
    /// Reader-facing prose the audits read with the narrative.
    #[must_use]
    pub fn prose(&self) -> Option<&str> {
        match self {
            Self::Claim { how, .. } => Some(how),
            Self::CostModel { note, .. } => note.as_deref(),
            Self::Layout { .. } => None,
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
        }
    }

    /// What is wrong with this explainer, as sentences an agent can act on.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut need = |ok: bool, problem: &str| {
            if !ok {
                problems.push(problem.to_string());
            }
        };
        match self {
            Self::Claim {
                claim,
                how,
                result,
                share,
            } => {
                need(!claim.trim().is_empty(), "a claim needs the claim, quoted");
                need(
                    result.is_none()
                        || share
                            .as_deref()
                            .is_some_and(|share| !share.trim().is_empty()),
                    "a claim that cites a measured `result` needs `share`: this step's part of it, \
                     such as \"about 36 MiB of the 573 MiB drop\"; a result belongs to the whole change",
                );
                need(
                    claim.chars().count() <= 400,
                    "quote at most 400 characters of the claim",
                );
                need(
                    !how.trim().is_empty(),
                    "a claim needs `how` this step serves it",
                );
            }
            Self::Layout {
                title,
                rows,
                source,
            } => {
                need(!title.trim().is_empty(), "a layout needs a title");
                need(
                    !source.trim().is_empty(),
                    "a layout needs the source of its sizes",
                );
                need(
                    (1..=MAX_ROWS).contains(&rows.len()),
                    "a layout has between 1 and 4 rows",
                );
                for row in rows {
                    need(
                        (1..=MAX_FIELDS).contains(&row.fields.len()),
                        "each layout row has between 1 and 8 fields",
                    );
                    need(
                        row.fields
                            .iter()
                            .all(|field| (1..=4096).contains(&field.bytes)),
                        "each layout field is between 1 and 4096 bytes",
                    );
                }
            }
            Self::CostModel {
                title,
                params,
                series,
                source,
                ..
            } => {
                need(!title.trim().is_empty(), "a cost model needs a title");
                need(
                    !source.trim().is_empty(),
                    "a cost model needs the source of its numbers",
                );
                need(
                    (1..=MAX_PARAMS).contains(&params.len()),
                    "a cost model has between 1 and 3 parameters",
                );
                need(
                    (1..=MAX_SERIES).contains(&series.len()),
                    "a cost model has between 1 and 4 series",
                );
                problems.extend(cost_model_problems(params, series));
            }
        }
        problems
    }
}

fn cost_model_problems(params: &[Param], series: &[Series]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for param in params {
        if !formula::is_name(&param.id) {
            problems.push(format!(
                "parameter id `{}` must be lowercase letters, digits, and _",
                param.id
            ));
        }
        if !ids.insert(param.id.as_str()) {
            problems.push(format!("parameter `{}` is declared twice", param.id));
        }
        if !(param.min <= param.default && param.default <= param.max) {
            problems.push(format!(
                "parameter `{}` needs min <= default <= max",
                param.id
            ));
        }
        if param
            .marks
            .iter()
            .any(|mark| *mark < param.min || *mark > param.max)
        {
            problems.push(format!(
                "parameter `{}` has a mark outside min..max",
                param.id
            ));
        }
        if param.scale == Scale::Log2 && param.min <= 0 {
            problems.push(format!(
                "log2 parameter `{}` needs a positive min",
                param.id
            ));
        }
    }
    let names: Vec<&str> = params.iter().map(|param| param.id.as_str()).collect();
    for series in series {
        let expression = match formula::Expr::parse(&series.formula, &names) {
            Ok(expression) => expression,
            Err(error) => {
                problems.push(format!("series `{}`: {error}", series.label));
                continue;
            }
        };
        for pick in [Pick::Min, Pick::Default, Pick::Max] {
            let values: Vec<f64> = params.iter().map(|param| pick.of(param)).collect();
            let value = expression.eval(&values);
            if !value.is_finite() || value < 0.0 {
                problems.push(format!(
                    "series `{}` is {value} at the parameters' {}; costs are finite and not negative",
                    series.label,
                    pick.name()
                ));
                break;
            }
        }
    }
    problems
}

#[derive(Clone, Copy)]
enum Pick {
    Min,
    Default,
    Max,
}

impl Pick {
    #[allow(clippy::cast_precision_loss)]
    fn of(self, param: &Param) -> f64 {
        (match self {
            Self::Min => param.min,
            Self::Default => param.default,
            Self::Max => param.max,
        }) as f64
    }

    fn name(self) -> &'static str {
        match self {
            Self::Min => "minimums",
            Self::Default => "defaults",
            Self::Max => "maximums",
        }
    }
}
