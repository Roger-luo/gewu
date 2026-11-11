//! Test sections Gewu writes itself, so no agent time goes to walking through
//! test code. Each chunk shows whole tests where they fit, named, with their
//! doc comments as the prose; the reviewer reads the code itself.
//!
//! Asked for by the reviewer after timing Cursor on #276: a third of the
//! agent's time went to sections that only walked through tests.

mod pack;
mod scan;

use std::collections::HashMap;

use pack::{Piece, Role, balanced, pieces, quotes, whole};

use super::{Expansion, HARNESS_SCHEMA_VERSION, WorkItem};
use crate::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, ReviewChunk, ReviewInput, chunk_height,
};

/// Names written as a short list: "`a`", "`a` and `b`", "`a`, `b`, and 2 more".
fn listed(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => format!("`{one}`"),
        [first, second] => format!("`{first}` and `{second}`"),
        [first, second, rest @ ..] => format!("`{first}`, `{second}`, and {} more", rest.len()),
    }
}

fn chunk(
    item: &WorkItem,
    index: usize,
    pieces: &[Piece],
    changes: &HashMap<&String, &Change>,
) -> ReviewChunk {
    let named = |role: Role| -> Vec<&str> {
        pieces
            .iter()
            .filter(|piece| piece.role == role)
            .filter_map(|piece| piece.name.as_deref())
            .collect()
    };
    let (tests, helpers) = (named(Role::Test), named(Role::Helper));
    let edits = pieces.iter().any(|piece| piece.role == Role::Edit);
    let path = pieces
        .first()
        .and_then(|piece| changes.get(&piece.change_id))
        .map_or("the tests", |change| change.path.as_str());
    let title = if !tests.is_empty() {
        format!(
            "{} {}",
            if tests.len() == 1 { "Test" } else { "Tests" },
            listed(&tests)
        )
    } else if !helpers.is_empty() {
        format!("Test helpers {}", listed(&helpers))
    } else if edits {
        format!("Changes to existing tests in `{path}`")
    } else {
        format!("Test setup in `{path}`")
    };
    let mut prose: Vec<String> = pieces
        .iter()
        .filter_map(|piece| piece.module_doc.clone())
        .collect();
    if index == 0 {
        prose.push(
            "Gewu wrote this section from the test code itself, without an agent; turn on \
             **Explain tests** in Settings to have the agent walk through tests."
                .to_owned(),
        );
    }
    let documented: Vec<String> = pieces
        .iter()
        .filter(|piece| piece.role == Role::Test)
        .filter_map(|piece| {
            let name = piece.name.as_deref()?;
            Some(match &piece.doc {
                Some(doc) => format!("- `{name}`: {doc}"),
                None => format!("- `{name}`"),
            })
        })
        .collect();
    if !documented.is_empty() {
        prose.push(documented.join("\n"));
    }
    if !helpers.is_empty() {
        prose.push(format!("Helpers the tests use: {}.", listed(&helpers)));
    }
    if edits {
        prose.push("Edits to code already in the file; the diff shows what changed.".to_owned());
    }
    if tests.is_empty() && helpers.is_empty() && !edits {
        prose.push("Imports and setup the tests share.".to_owned());
    }
    let added = pieces.iter().all(|piece| piece.added);
    let mut parts = Vec::new();
    match tests.len() {
        0 => {}
        1 => parts.push("One test".to_owned()),
        count => parts.push(format!("{count} tests")),
    }
    if !helpers.is_empty() {
        parts.push("helpers".to_owned());
    }
    if edits {
        parts.push("edits to existing tests".to_owned());
    }
    if parts.is_empty() {
        parts.push("Test setup".to_owned());
    }
    let mut summary = format!("{} in `{path}`", parts.join(", "));
    if let Some(first) = summary.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    summary.push_str(if added {
        ", shown as written."
    } else {
        ", as the diff changes them."
    });
    ReviewChunk {
        id: format!("{}-tests-{}", item.id, index + 1),
        title,
        summary,
        kind: ChunkKind::Test,
        content: vec![
            ChunkBlock::Narrative {
                markdown: prose.join("\n\n"),
            },
            ChunkBlock::Changes {
                quotes: quotes(pieces),
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

/// The expansion Gewu writes for an item of tests, or `None` when it does
/// not validate, so an agent writes the item instead.
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
    // The plan's `tests` mark is the agent's word; an item that adds no test
    // is not one Gewu writes. Found by reviewing the test-section change.
    if !all.iter().any(|piece| piece.role == Role::Test) {
        return None;
    }
    let fitting = whole(&all, &fits);
    let groups = match fitting {
        groups if groups.len() <= max_chunks => groups,
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
