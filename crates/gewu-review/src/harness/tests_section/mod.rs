//! Test sections Gewu writes itself, so no agent time goes to walking through
//! test code. Each chunk shows whole tests where they fit, named, with their
//! doc comments as the prose; the reviewer reads the code itself.
//!
//! Asked for by the reviewer after timing Cursor on #276: a third of the
//! agent's time went to sections that only walked through tests.

mod pack;
mod scan;

use std::collections::HashMap;

use pack::{Piece, balanced, fitted, pieces, quotes};

use super::{Expansion, HARNESS_SCHEMA_VERSION, WorkItem};
use crate::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, ReviewChunk, ReviewInput, chunk_height,
};

fn chunk(
    item: &WorkItem,
    index: usize,
    pieces: &[Piece],
    changes: &HashMap<&String, &Change>,
) -> ReviewChunk {
    let names: Vec<&str> = pieces
        .iter()
        .filter_map(|piece| piece.name.as_deref())
        .collect();
    let path = pieces
        .first()
        .and_then(|piece| changes.get(&piece.change_id))
        .map_or("the tests", |change| change.path.as_str());
    let title = match names.as_slice() {
        [] => format!("Test support in `{path}`"),
        [one] => format!("Test `{one}`"),
        [first, second] => format!("Tests `{first}` and `{second}`"),
        [first, second, rest @ ..] => {
            format!("Tests `{first}`, `{second}`, and {} more", rest.len())
        }
    };
    let mut prose: Vec<String> = Vec::new();
    if index == 0 {
        prose.push(
            "Gewu wrote this section from the test code itself, without an agent; turn on \
             **Explain tests** in Settings to have the agent walk through tests."
                .to_owned(),
        );
    }
    let listed: Vec<String> = pieces
        .iter()
        .filter_map(|piece| {
            let name = piece.name.as_deref()?;
            Some(match &piece.doc {
                Some(doc) => format!("- `{name}`: {doc}"),
                None => format!("- `{name}`"),
            })
        })
        .collect();
    if listed.is_empty() {
        prose.push("Helpers and setup the tests share.".to_owned());
    } else {
        prose.push(listed.join("\n"));
    }
    let quotes = quotes(pieces);
    let added = quotes.iter().all(|quote| {
        changes.get(&quote.change_id).is_some_and(|change| {
            change
                .body(quote.start, quote.end)
                .iter()
                .all(|line| line.starts_with('+'))
        })
    });
    ReviewChunk {
        id: format!("{}-tests-{}", item.id, index + 1),
        title,
        summary: format!(
            "{} in `{path}`, shown as written.",
            match names.len() {
                0 => "Test support".to_owned(),
                1 => "One test".to_owned(),
                count => format!("{count} tests"),
            }
        ),
        kind: ChunkKind::Test,
        content: vec![
            ChunkBlock::Narrative {
                markdown: prose.join("\n\n"),
            },
            ChunkBlock::Changes {
                quotes,
                presentation: if added {
                    ChangePresentation::AddedCode
                } else {
                    ChangePresentation::Diff
                },
            },
        ],
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    }
}

/// The expansion Gewu writes for an item of tests, or `None` when the code
/// does not split into at most `max_chunks` chunks that fit, so an agent
/// writes it instead.
#[must_use]
pub fn test_expansion(
    input: &ReviewInput,
    item: &WorkItem,
    max_chunks: usize,
) -> Option<Expansion> {
    let changes: HashMap<&String, &Change> = input
        .changes
        .iter()
        .map(|change| (&change.id, change))
        .collect();
    let fits = |index: usize, pieces: &[Piece]| {
        chunk_height(&chunk(item, index, pieces, &changes), &changes) <= input.max_chunk_lines
    };
    let mut all = Vec::new();
    for quote in &item.covers {
        all.extend(pieces(changes.get(&quote.change_id)?, quote));
    }
    let groups = match fitted(&all, &fits) {
        Some(groups) if groups.len() <= max_chunks => groups,
        // Found by running Cursor on #276: test items of 200 to 370 lines,
        // which the plan accepts, do not fit six chunks of the row budget.
        // Whole tests are shown instead, in at most `max_chunks` views that
        // scroll, rather than handing the item to an agent.
        _ => balanced(&all, max_chunks),
    };
    let chunks = groups
        .iter()
        .enumerate()
        .map(|(index, pieces)| {
            let mut chunk = chunk(item, index, pieces, &changes);
            if chunk_height(&chunk, &changes) > input.max_chunk_lines {
                chunk.oversized_reason = Some(
                    "Gewu shows these tests whole rather than cut mid-test; the view scrolls."
                        .to_owned(),
                );
            }
            chunk
        })
        .collect();
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks,
    };
    expansion
        .validate(input, item, max_chunks)
        .ok()
        .map(|()| expansion)
}

#[cfg(test)]
mod tests;
