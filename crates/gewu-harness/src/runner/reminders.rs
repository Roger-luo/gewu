//! Reminders printed when a draft is measured: not rejections, but what a
//! reviewer would ask for that the draft does not yet give.

use gewu_review::{ChunkBlock, ChunkKind, Explainer, ReviewChunk};

use super::HarnessRunner;

/// The explainers in `chunks`, in order.
fn explainers<'a>(chunks: impl IntoIterator<Item = &'a ReviewChunk>) -> Vec<&'a Explainer> {
    chunks
        .into_iter()
        .flat_map(|chunk| &chunk.content)
        .filter_map(|block| match block {
            ChunkBlock::Explainer(explainer) => Some(explainer),
            _ => None,
        })
        .collect()
}

/// Numbers in `text` with the unit written after each: "24 MiB of the
/// 32-byte word" gives ("24", "MiB") and ("32", "-byte").
fn amounts(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
            .unwrap_or(tail.len());
        let number = tail[..end].trim_end_matches(['.', ',']).replace(',', "");
        let after = tail[end..].trim_start();
        let unit: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, ')' | ';' | '/'))
            .collect();
        found.push((number, unit.trim_end_matches(['.', ',']).to_owned()));
        rest = &tail[end..];
    }
    found
}

/// The amount a claim's share credits to its step: the first number in
/// `share` written in one of the result's units and not one of the result's
/// own numbers, so "enables the 32-byte layout (24 MiB saved …)" gives "24"
/// for a result in MiB.
///
/// Found by QA of Cursor's run E of #276: taking the first number read "32"
/// there, and missed that the share repeated an earlier section's 24 MiB.
pub(super) fn credited(explainer: &Explainer) -> Option<String> {
    let Explainer::Claim {
        result: Some(result),
        share: Some(share),
        ..
    } = explainer
    else {
        return None;
    };
    let whole = amounts(result);
    amounts(share)
        .into_iter()
        .find(|(number, unit)| {
            whole.iter().any(|(_, whole_unit)| whole_unit == unit)
                && !whole.iter().any(|(whole_number, _)| whole_number == number)
        })
        .map(|(number, unit)| format!("{number} {unit}"))
}

/// Explainers a walkthrough should use somewhere, with when they fit. Each is
/// suggested until some section draws one. Found by QA of Cursor's runs of
/// #276: no stepper in sixteen sections once drafts had layouts, and no
/// timeline in any run, though the change pools a workspace across steps.
const UNTIL_DRAWN: [(&str, &str); 2] = [
    (
        "stepper",
        "No section so far steps through an algorithm. If this item has code a reader would \
         simulate in their head — a compaction, a heap sift, a probe or a bound-and-prune loop \
         — show it with a `stepper`.",
    ),
    (
        "timeline",
        "No section so far draws a `timeline`. If this item builds or allocates something once \
         that was built in every step, or pools what was reallocated, show the steps with one.",
    ),
];

impl HarnessRunner {
    /// A reminder, not a rejection, when the author's description makes
    /// claims and the draft does not yet show how this item delivers them.
    /// Found by running Cursor on #276: every section's purpose named the
    /// claim it delivered, and the draft still told "32 bytes instead of 48"
    /// in prose alone.
    pub(super) fn explainer_reminder(&self, chunks: &[ReviewChunk]) -> String {
        let Ok(description) = std::fs::read_to_string(
            self.input_path
                .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        ) else {
            return String::new();
        };
        // Found by QA of Cursor's #276 run: the reminder asked for claims and
        // layouts on all thirteen test sections, where neither fits.
        if chunks.iter().all(|chunk| chunk.kind == ChunkKind::Test) {
            return String::new();
        }
        // Found by QA of the full #276 walkthrough: Cursor marked 45 of its
        // 46 chunks oversized, so almost every view scrolled.
        let oversized = chunks
            .iter()
            .filter(|chunk| chunk.oversized_reason.is_some())
            .count();
        let scrolls = (oversized >= 2 && oversized * 2 > chunks.len()).then(|| {
            format!(
                "\n{oversized} of {} chunks here are marked over their budget with \
                 `oversized_reason`, so the reviewer takes in more at once than the budget allows. \
                 A view can almost always be made smaller: split where the code's subject \
                 changes, and keep `oversized_reason` for a unit that cannot be split.\n",
                chunks.len()
            )
        });
        let drafted = explainers(chunks);
        let scrolls = scrolls.unwrap_or_default();
        if drafted.is_empty() {
            return scrolls
                + "\nNo chunk has an explainer. `description.md` makes claims: if this item \
                    delivers one, quote it in a `claim` block on its first chunk, and show it: a \
                    record with a `layout`, a cost that scales with a `cost_model`, an algorithm \
                    a reader would simulate with a `stepper`, work moved out of a loop with a \
                    `timeline` (see the expand prompt). If it delivers none, say what it enables \
                    instead.\n";
        }
        let earlier = self.spec_path.exists().then(|| self.spec().ok()).flatten();
        let before = earlier
            .as_ref()
            .map(|spec| explainers(&spec.chunks))
            .unwrap_or_default();
        let mut notes = Vec::new();
        // Found by QA of Cursor's #276 run: 21 claim blocks restating the
        // description, and not one layout or timeline in 38 sections.
        if drafted
            .iter()
            .all(|explainer| matches!(explainer, Explainer::Claim { .. }))
        {
            notes.push(
                "Every explainer here is a `claim`. A claim quotes; it does not show. If this \
                 item shrinks a record, scales a cost, steps through an algorithm, or moves work \
                 out of a loop, show it with a `layout`, `cost_model`, `stepper`, or `timeline`."
                    .to_owned(),
            );
        }
        // Found by the same QA: no section stated its share of a measured result.
        let measured = description.contains('%') || description.contains("MiB");
        let shared = drafted
            .iter()
            .any(|explainer| matches!(explainer, Explainer::Claim { share: Some(_), .. }));
        if measured && !shared {
            notes.push(
                "`description.md` reports measured results and no chunk states its share. If \
                 this item delivers part of one, say how much, as an estimate when the change \
                 measures no part on its own (a `claim` with `result` and `share`). A share is \
                 this item's part of that result, never what its own code costs; if it delivers \
                 none, leave `share` out."
                    .to_owned(),
            );
        }
        // Found by QA of Cursor's run C of #276: a later section credited the
        // same 24 MiB as an earlier one, and neither said so.
        let counted: Vec<String> = before.iter().filter_map(|e| credited(e)).collect();
        let repeated: Vec<String> = drafted
            .iter()
            .filter_map(|e| credited(e))
            .filter(|amount| counted.contains(amount))
            .collect();
        if !repeated.is_empty() {
            notes.push(format!(
                "A share here credits the same amount as an earlier section's share ({}). If it \
                 is the same saving, say so in `how` and credit it once, so the shares are not \
                 added twice.",
                repeated.join(", ")
            ));
        }
        let everything: Vec<&Explainer> = before.iter().chain(&drafted).copied().collect();
        notes.extend(super::tally::overcredited(&everything));
        // Found by QA of Cursor's runs E and F of #276: a 104 MiB saving in
        // the exponential and a 56 MiB one at admission were offered toward
        // one peak, and neither said which phase it fell in.
        let unplaced = drafted.iter().any(|explainer| {
            matches!(explainer, Explainer::Claim { share: Some(share), how, .. }
                if !format!("{share} {how}").to_lowercase().contains("peak"))
        });
        if unplaced {
            notes.push(
                "A share here does not say whether its saving falls at the peak. Say in `how` \
                 which phase the structure lives in and whether that is where the result was \
                 measured; savings in different phases do not add up toward one peak."
                    .to_owned(),
            );
        }
        for (kind, note) in UNTIL_DRAWN {
            let drawn = |explainer: &&Explainer| explainer.kind() == kind;
            if !before.iter().any(drawn) && !drafted.iter().any(drawn) {
                notes.push(note.to_owned());
            }
        }
        let notes = if notes.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", notes.join("\n\n"))
        };
        format!("{scrolls}{notes}")
    }
}

#[cfg(test)]
mod tests {
    use gewu_review::Explainer;

    use super::credited;

    fn claim(share: &str) -> Explainer {
        Explainer::Claim {
            claim: "Peak memory falls".into(),
            result: Some("940 → 367 MiB (−61%)".into()),
            share: Some(share.into()),
            how: "How.".into(),
        }
    }

    /// Found by QA of Cursor's run E of #276.
    #[test]
    fn a_share_credits_its_first_amount_in_the_results_unit() {
        let enables = claim("enables the 32-byte `Word` layout (24 MiB saved across 3·B words)");
        assert_eq!(credited(&enables).as_deref(), Some("24 MiB"));
        let part = claim("estimated 104 MiB saved (a 2^21-bucket table shrinks from 114 MiB)");
        assert_eq!(credited(&part).as_deref(), Some("104 MiB"));
        let whole = claim("all 940 MiB");
        assert_eq!(credited(&whole), None);
    }
}
