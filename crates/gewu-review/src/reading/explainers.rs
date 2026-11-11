//! What an explainer costs to take in, and what it saves on the code beside it.

use std::collections::{HashMap, HashSet};

use crate::{Explainer, explainer::line_ranges, load::Rates};

use super::words as prose;

/// The explainer's own reading seconds, and the seconds it saves on quoted
/// added lines (`added`: trimmed text to cost).
pub(super) fn cost(
    explainer: &Explainer,
    added: &HashMap<String, f64>,
    rates: &Rates,
) -> (f64, f64) {
    let words = |text: &str| prose(text, rates);
    match explainer {
        Explainer::Claim {
            claim,
            result,
            share,
            how,
        } => {
            let extra = result.as_deref().map_or(0.0, words) + share.as_deref().map_or(0.0, words);
            (rates.value + words(claim) + words(how) + extra, 0.0)
        }
        Explainer::Layout { rows, source, .. } => {
            let fields: usize = rows.iter().map(|row| row.fields.len()).sum();
            let notes: f64 = rows
                .iter()
                .flat_map(|row| &row.fields)
                .filter_map(|field| field.note.as_deref())
                .map(words)
                .sum();
            let own = rates.figure + rows.len() as f64 * rates.value + fields as f64 * rates.value;
            (own + notes + words(source), 0.0)
        }
        Explainer::CostModel {
            params,
            series,
            note,
            ..
        } => {
            let own = rates.figure
                + params.len() as f64 * rates.cost_model_param
                + series.len() as f64 * rates.cost_model_param;
            (own + note.as_deref().map_or(0.0, words), 0.0)
        }
        Explainer::Timeline { lanes, .. } => {
            let work: usize = lanes.iter().map(|lane| lane.work.len()).sum();
            let own = rates.figure + lanes.len() as f64 * rates.value + work as f64 * rates.value;
            (own, 0.0)
        }
        Explainer::Stepper { steps, .. } => {
            let mut own = rates.figure;
            let mut discount = 0.0;
            // Lines read already, in the quotes or an earlier step.
            let mut read: HashSet<&str> = HashSet::new();
            let mut walked: HashSet<&str> = HashSet::new();
            for step in steps {
                own += rates.stepper_step
                    + words(&step.caption)
                    + step.state.len() as f64 * rates.value;
                let highlighted: usize = step
                    .highlight
                    .as_deref()
                    .and_then(|text| line_ranges(text).ok())
                    .map_or(0, |ranges| {
                        ranges.iter().map(|(start, end)| end - start + 1).sum()
                    });
                own += highlighted as f64 * rates.highlighted_line;
                for line in step.code.as_deref().unwrap_or_default().lines() {
                    let line = line.trim();
                    if line.is_empty() || !read.insert(line) {
                        continue;
                    }
                    match added.get(line) {
                        Some(cost) if walked.insert(line) => {
                            discount += cost * rates.stepper_discount
                        }
                        Some(_) => {}
                        None => own += rates.unquoted_stepper_line,
                    }
                }
            }
            (own, discount)
        }
    }
}
