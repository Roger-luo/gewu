//! Views of prose files that end in the middle of a sentence.
//!
//! A self-review judge found a chunk boundary splitting a bullet of a skill
//! document between two stops. In Markdown that is visible from the text
//! alone: the view's last line does not end a sentence and the next line
//! continues it.

use serde::Serialize;

use crate::{Change, ValidatedReview};

/// A view of a Markdown change whose last line runs on into the next.
#[derive(Clone, Debug, Serialize)]
pub struct SplitSentence {
    pub chunk_id: String,
    pub path: String,
    /// Body line the view ends on.
    pub line: usize,
}

pub(super) fn check(review: &ValidatedReview) -> Vec<SplitSentence> {
    let mut found = Vec::new();
    for chunk in &review.spec().chunks {
        for quote in chunk.quotes() {
            let Some(change) = review
                .input()
                .changes
                .iter()
                .find(|change| change.id == quote.change_id)
            else {
                continue;
            };
            if change.path.ends_with(".md") && runs_on(change, quote.end) {
                found.push(SplitSentence {
                    chunk_id: chunk.id.clone(),
                    path: change.path.clone(),
                    line: quote.end,
                });
            }
        }
    }
    found
}

/// Whether body line `end` stops mid-sentence with the sentence going on below.
fn runs_on(change: &Change, end: usize) -> bool {
    let text = |line: usize| {
        change
            .body(line, line)
            .first()
            .map(|text| text.get(1..).unwrap_or_default().trim().to_owned())
    };
    let (Some(last), Some(next)) = (text(end), text(end + 1)) else {
        return false;
    };
    let ends = last.is_empty() || last.ends_with(['.', ':', '!', '?', '`', '|', ')']);
    let starts_block = next.is_empty()
        || next.starts_with(['#', '-', '*', '>', '|', '`'])
        || next.starts_with(|c: char| c.is_ascii_digit());
    !ends && !starts_block
}

#[cfg(test)]
mod tests {
    use super::runs_on;
    use crate::Change;

    #[test]
    fn a_view_ending_inside_a_wrapped_bullet_runs_on() {
        let change = Change::new(
            "SKILL.md",
            (0, 0, 1, 4),
            "@@ -0,0 +1,4 @@\n+- Demand a friction log with exact commands\n+  and exact output.\n+\n+## Next\n",
        );
        assert!(runs_on(&change, 1));
        assert!(!runs_on(&change, 2));
        assert!(!runs_on(&change, 3));
    }
}
