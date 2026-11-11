//! Test sections Gewu writes itself, so no agent time goes to walking through
//! test code. Each chunk shows whole tests where they fit, named, with their
//! doc comments as the prose; the reviewer reads the code itself.
//!
//! Asked for by the reviewer after timing Cursor on #276: a third of the
//! agent's time went to sections that only walked through tests.

mod scan;

use std::collections::HashMap;

use super::{Expansion, HARNESS_SCHEMA_VERSION, WorkItem};
use crate::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Quote, ReviewChunk, ReviewInput,
    chunk_height,
};

/// One test, or the code between tests, within one change.
#[derive(Clone)]
struct Piece {
    change_id: String,
    start: usize,
    end: usize,
    name: Option<String>,
    doc: Option<String>,
}

/// The pieces a quote of test code falls into, split where each test begins
/// (its doc comment and attributes included).
fn pieces(change: &Change, quote: &Quote) -> Vec<Piece> {
    let lines = change.body(quote.start, quote.end);
    let mut starts = vec![0];
    for index in 1..lines.len() {
        // The first line of a test's attributes, after any doc comment.
        if scan::starts_test(lines[index]) && !scan::starts_test(lines[index - 1]) {
            let last = *starts.last().unwrap_or(&0);
            let mut start = index;
            while start > last + 1 && scan::leads_test(lines[start - 1]) {
                start -= 1;
            }
            if start > last {
                starts.push(start);
            }
        }
    }
    starts.push(lines.len());
    starts
        .windows(2)
        .filter(|pair| pair[1] > pair[0])
        .map(|pair| {
            let span = &lines[pair[0]..pair[1]];
            Piece {
                change_id: change.id.clone(),
                start: quote.start + pair[0],
                end: quote.start + pair[1] - 1,
                name: scan::test_name(span),
                doc: scan::doc(span),
            }
        })
        .collect()
}

/// Quotes for consecutive pieces, joining those that continue one another.
fn quotes(pieces: &[Piece]) -> Vec<Quote> {
    let mut quotes: Vec<Quote> = Vec::new();
    for piece in pieces {
        match quotes.last_mut() {
            Some(last) if last.change_id == piece.change_id && last.end + 1 == piece.start => {
                last.end = piece.end;
            }
            _ => quotes.push(Quote {
                change_id: piece.change_id.clone(),
                start: piece.start,
                end: piece.end,
            }),
        }
    }
    quotes
}

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
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    for quote in &item.covers {
        let change = changes.get(&quote.change_id)?;
        for piece in pieces(change, quote) {
            let mut trial = current.clone();
            trial.push(piece.clone());
            if fits(groups.len(), &trial) {
                current = trial;
                continue;
            }
            if !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
            // A test too long for one chunk is shown in consecutive parts.
            let mut rest = piece;
            while !fits(groups.len(), std::slice::from_ref(&rest)) {
                let line = Piece {
                    end: rest.start,
                    ..rest.clone()
                };
                if !fits(groups.len(), std::slice::from_ref(&line)) {
                    return None;
                }
                let mut end = rest.start;
                while end < rest.end
                    && fits(
                        groups.len(),
                        &[Piece {
                            end: end + 1,
                            ..rest.clone()
                        }],
                    )
                {
                    end += 1;
                }
                groups.push(vec![Piece {
                    end,
                    ..rest.clone()
                }]);
                rest = Piece {
                    start: end + 1,
                    ..rest
                };
            }
            current = vec![rest];
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks: groups
            .iter()
            .enumerate()
            .map(|(index, pieces)| chunk(item, index, pieces, &changes))
            .collect(),
    };
    expansion
        .validate(input, item, max_chunks)
        .ok()
        .map(|()| expansion)
}

#[cfg(test)]
mod tests;
