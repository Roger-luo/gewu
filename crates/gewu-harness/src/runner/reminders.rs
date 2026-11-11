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

/// The amount a claim's share credits to its step: the first number in
/// `share` that is not the measured result's own, so "about 24 MiB of the
/// 573 MiB drop" gives "24" and not the total it is a part of.
fn credited(explainer: &Explainer) -> Option<String> {
    let Explainer::Claim {
        result,
        share: Some(share),
        ..
    } = explainer
    else {
        return None;
    };
    let numbers = |text: &str| -> Vec<String> {
        text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .map(|part| part.trim_matches('.').to_owned())
            .filter(|part| !part.is_empty())
            .collect()
    };
    let whole = result.as_deref().map(numbers).unwrap_or_default();
    numbers(share)
        .into_iter()
        .find(|amount| !whole.contains(amount))
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
        let drafted = explainers(chunks);
        if drafted.is_empty() {
            return "\nNo chunk has an explainer. `description.md` makes claims: if this item \
                    delivers one, quote it in a `claim` block on its first chunk, and show it: a \
                    record with a `layout`, a cost that scales with a `cost_model`, an algorithm \
                    a reader would simulate with a `stepper`, work moved out of a loop with a \
                    `timeline` (see the expand prompt). If it delivers none, say what it enables \
                    instead.\n"
                .to_owned();
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
        for (kind, note) in UNTIL_DRAWN {
            let drawn = |explainer: &&Explainer| explainer.kind() == kind;
            if !before.iter().any(drawn) && !drafted.iter().any(drawn) {
                notes.push(note.to_owned());
            }
        }
        if notes.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", notes.join("\n\n"))
        }
    }
}
