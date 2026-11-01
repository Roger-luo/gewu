//! Whether every line number the prose cites is one the reader can see.
//!
//! Quotes name hunk-body lines, but the reviewer's gutter shows file lines, so
//! prose must cite those. A self-review author trimmed a view and left its
//! prose citing lines no longer on screen; nothing caught it until now.

use std::collections::HashSet;

use serde::Serialize;

use super::ChunkProse;
use crate::{Change, ReviewChunk, ValidatedReview};

/// A cited line that no view of the chunk shows in its gutter.
#[derive(Clone, Debug, Serialize)]
pub struct InvisibleLine {
    pub chunk_id: String,
    /// The citation as written, such as `lines 27-28`.
    pub citation: String,
}

pub(super) fn check(review: &ValidatedReview) -> Vec<InvisibleLine> {
    review
        .spec()
        .chunks
        .iter()
        .flat_map(|chunk| {
            invisible_in(chunk, &review.input().changes)
                .into_iter()
                .map(|citation| InvisibleLine {
                    chunk_id: chunk.id.clone(),
                    citation,
                })
        })
        .collect()
}

/// Citations in one chunk's prose that none of its views shows. Advisory, not
/// a defect by itself: prose may rightly cite a line an earlier chunk showed,
/// or unchanged code outside the diff. Drift after trimming a view is the case
/// worth catching, so authors see these while drafting.
pub fn invisible_in(chunk: &ReviewChunk, changes: &[Change]) -> Vec<String> {
    let shown = shown_lines(chunk, changes);
    cited(&ChunkProse::new(chunk).text())
        .into_iter()
        .filter(|(_, start, end)| !(*start..=*end).all(|line| shown.contains(&line)))
        .map(|(citation, _, _)| citation)
        .collect()
}

/// File lines the chunk's views put in the gutter. Removed lines have none.
fn shown_lines(chunk: &ReviewChunk, changes: &[Change]) -> HashSet<u32> {
    let mut shown = HashSet::new();
    for quote in chunk.quotes() {
        if let Some(change) = changes.iter().find(|change| change.id == quote.change_id) {
            shown.extend((quote.start..=quote.end).filter_map(|line| change.new_line_at(line)));
        }
    }
    shown
}

/// `line N` and `lines N-M` citations, skipping ones that name another file
/// (`line 104 of view.rs`, `line 12 in spec.rs`), which point outside the views.
fn cited(text: &str) -> Vec<(String, u32, u32)> {
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = lower[from..].find("line") {
        let at = from + offset;
        from = at + 4;
        if lower[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let rest = &lower[at + 4..];
        let rest = rest.strip_prefix('s').unwrap_or(rest);
        let Some(rest) = rest.strip_prefix(' ') else {
            continue;
        };
        let (start, rest) = number(rest);
        let Some(start) = start else { continue };
        let (end, tail) = match rest.strip_prefix(['-', '–']) {
            Some(after) => match number(after) {
                (Some(end), tail) => (end, tail),
                _ => (start, rest),
            },
            None => (start, rest),
        };
        if tail.trim_start().starts_with("of ") || tail.trim_start().starts_with("in ") {
            continue;
        }
        // `help.rs` line 26: a file named just before the citation.
        let before = lower[..at].trim_end();
        if before.ends_with('`') && before[..before.len() - 1].contains('.') {
            let name = before[..before.len() - 1]
                .rsplit('`')
                .next()
                .unwrap_or_default();
            if name.contains('.') && !name.contains(' ') {
                continue;
            }
        }
        let length = lower.len() - tail.len() - at;
        found.push((text[at..at + length].to_owned(), start, end.max(start)));
    }
    found
}

fn number(text: &str) -> (Option<u32>, &str) {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    (text[..digits].parse().ok(), &text[digits..])
}

#[cfg(test)]
mod tests {
    use super::cited;

    #[test]
    fn citations_are_lines_and_ranges_that_name_no_other_file() {
        let found = cited(
            "Compare lines 4-8 with line 12. Line 104 of view.rs differs; `help.rs` line 26 too; baseline 3 is not one.",
        );
        let found: Vec<_> = found.iter().map(|(_, start, end)| (*start, *end)).collect();
        assert_eq!(found, [(4, 8), (12, 12)]);
    }
}
