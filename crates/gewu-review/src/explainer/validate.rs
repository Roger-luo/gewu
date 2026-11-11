//! What makes an explainer usable, and claims checked against the author's
//! description.

use super::{Explainer, Param, Scale, Series, Step, formula, line_ranges};

/// Claims in `chunks` that the author's `description` does not contain word
/// for word (whitespace aside), as errors an agent can act on. With no
/// description, every claim is refused: there is nothing to quote.
#[must_use]
pub fn unquoted_claims<'a>(
    chunks: impl IntoIterator<Item = &'a crate::ReviewChunk>,
    description: Option<&str>,
) -> Vec<String> {
    // Whitespace, and typographic forms an agent may not reproduce byte for
    // byte (dashes, minus signs, middle dots, curly quotes), compare equal.
    let squash = |text: &str| {
        text.chars()
            .map(|c| match c {
                '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
                '\u{00B7}' | '\u{22C5}' | '\u{2219}' => '*',
                '\u{2018}' | '\u{2019}' => '\'',
                '\u{201C}' | '\u{201D}' => '"',
                '\u{00A0}' | '\u{202F}' => ' ',
                c => c,
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
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
const MAX_STEPS: usize = 12;
const MAX_STEP_LINES: usize = 30;
const MAX_FIELDS: usize = 8;
const MAX_PARAMS: usize = 3;
const MAX_SERIES: usize = 4;

impl Explainer {
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
            Self::Stepper {
                title,
                steps,
                start_line,
                ..
            } => {
                need(!title.trim().is_empty(), "a stepper needs a title");
                need(
                    (2..=MAX_STEPS).contains(&steps.len()),
                    "a stepper has between 2 and 12 steps",
                );
                problems.extend(stepper_problems(steps, start_line.unwrap_or(1)));
            }
            Self::Timeline {
                title,
                steps,
                step_label,
                lanes,
                ..
            } => {
                need(!title.trim().is_empty(), "a timeline needs a title");
                need(
                    !step_label.trim().is_empty(),
                    "a timeline needs a `step_label`",
                );
                need(
                    (2..=MAX_STEPS as u32).contains(steps),
                    "a timeline draws between 2 and 12 steps",
                );
                need(
                    (1..=3).contains(&lanes.len()),
                    "a timeline has between 1 and 3 lanes",
                );
                for lane in lanes {
                    need(
                        !lane.label.trim().is_empty() && (1..=4).contains(&lane.work.len()),
                        "each timeline lane has a label and between 1 and 4 pieces of work",
                    );
                    need(
                        lane.work.iter().all(|work| !work.label.trim().is_empty()),
                        "each piece of timeline work needs a label",
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

/// Columns a stepper line may use: wider code is clipped beside its caption.
const MAX_STEP_COLUMNS: usize = 80;

fn stepper_problems(steps: &[Step], first: usize) -> Vec<String> {
    let mut problems = Vec::new();
    if steps.first().is_some_and(|step| step.code.is_none()) {
        problems.push("a stepper's first step needs `code`; later steps may keep it".into());
    }
    let mut lines = 0;
    for (index, step) in steps.iter().enumerate() {
        let number = index + 1;
        if step.caption.trim().is_empty() {
            problems.push(format!("stepper step {number} needs a caption"));
        }
        if let Some(code) = &step.code {
            lines = code.lines().count();
            if lines == 0 || lines > MAX_STEP_LINES {
                problems.push(format!("stepper step {number} code has 1 to 30 lines"));
            }
            if code
                .lines()
                .any(|line| line.chars().count() > MAX_STEP_COLUMNS)
            {
                problems.push(format!(
                    "stepper step {number} has a line over {MAX_STEP_COLUMNS} columns; wrap it as \
                     rustfmt or prettier would, so nothing is clipped"
                ));
            }
        }
        let last = first + lines.saturating_sub(1);
        if let Some(highlight) = &step.highlight {
            match line_ranges(highlight) {
                Ok(ranges)
                    if ranges
                        .iter()
                        .any(|(start, end)| *start < first || *end > last) =>
                {
                    problems.push(format!(
                        "stepper step {number} highlights outside lines {first}-{last}, the code \
                         it shows (numbered from `start_line`)"
                    ));
                }
                Ok(_) => {}
                Err(error) => problems.push(format!("stepper step {number}: {error}")),
            }
        }
        if step.state.len() > 6 {
            problems.push(format!("stepper step {number} shows at most 6 values"));
        }
    }
    problems
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
