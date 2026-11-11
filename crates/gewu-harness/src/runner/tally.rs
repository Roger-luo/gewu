//! Shares added up against the result they are parts of: shares that credit
//! more than the result changed by double-count something, or fall in phases
//! that do not share one peak.

use gewu_review::Explainer;

use super::reminders::credited;

/// How much `result` changed: the two numbers around its arrow, as in
/// "940 → 367 MiB (−61%)", give 573 and the unit after the second.
fn change(result: &str) -> Option<(f64, String)> {
    let (before, after) = result.split_once('→').or_else(|| result.split_once("->"))?;
    let number = |text: &str| -> Option<f64> {
        let digits: String = text
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
            .filter(|c| *c != ',')
            .collect();
        digits.parse().ok()
    };
    let to = after.trim_start();
    let unit: String = to
        .chars()
        .skip_while(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
        .collect::<String>()
        .split_whitespace()
        .next()?
        .to_owned();
    Some((
        (number(before.trim_end().rsplit(' ').next()?)? - number(to)?).abs(),
        unit,
    ))
}

/// Notes for each measured result whose shares, across the walkthrough so far,
/// credit more than the result changed by.
///
/// Found by QA of the full #276 walkthrough: the ladder's memory shares added
/// up to 650–770 MiB against a measured 573 MiB drop.
pub(super) fn overcredited(explainers: &[&Explainer]) -> Vec<String> {
    let mut totals: Vec<(String, f64, String, f64)> = Vec::new();
    for explainer in explainers {
        let Explainer::Claim {
            result: Some(result),
            ..
        } = explainer
        else {
            continue;
        };
        let (Some((whole, unit)), Some(amount)) = (change(result), credited(explainer)) else {
            continue;
        };
        let Some(value) = amount
            .strip_suffix(&format!(" {unit}"))
            .and_then(|number| number.parse::<f64>().ok())
        else {
            continue;
        };
        match totals.iter_mut().find(|(name, ..)| name == result) {
            Some(entry) => entry.3 += value,
            None => totals.push((result.clone(), whole, unit, value)),
        }
    }
    totals
        .into_iter()
        .filter(|(_, whole, _, sum)| *sum > *whole * 1.05)
        .map(|(result, whole, unit, sum)| {
            format!(
                "The shares of \"{result}\" so far add up to about {sum:.0} {unit}, more than \
                 its {whole:.0} {unit} change. Some saving is counted twice, or falls in a phase \
                 that is not where the result was measured; say which, and credit only what \
                 this item adds at that moment."
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gewu_review::Explainer;

    use super::overcredited;

    fn share(amount: &str) -> Explainer {
        Explainer::Claim {
            claim: "Peak memory falls".into(),
            result: Some("940 → 367 MiB (−61%)".into()),
            share: Some(format!("about {amount} MiB of the 573 MiB drop")),
            how: "How.".into(),
        }
    }

    /// Found by QA of the full #276 walkthrough.
    #[test]
    fn shares_that_add_up_to_more_than_the_result_are_noted() {
        let fits = [share("300"), share("200")];
        assert!(overcredited(&fits.iter().collect::<Vec<_>>()).is_empty());
        let over = [share("300"), share("200"), share("150")];
        let notes = overcredited(&over.iter().collect::<Vec<_>>());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("about 650 MiB"), "{}", notes[0]);
        assert!(notes[0].contains("573 MiB change"), "{}", notes[0]);
    }
}
