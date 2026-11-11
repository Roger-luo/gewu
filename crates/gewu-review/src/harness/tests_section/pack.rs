//! Splitting test code into pieces, one test each, and packing them into
//! chunks: within the row budget where they fit, and otherwise whole tests
//! in at most the item's chunk cap.

use super::scan;
use crate::{Change, Quote};

/// One test, or the code between tests, within one change.
#[derive(Clone)]
pub(super) struct Piece {
    pub(super) change_id: String,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) name: Option<String>,
    pub(super) doc: Option<String>,
}

/// The pieces a quote of test code falls into, split where each test begins
/// (its doc comment and attributes included).
pub(super) fn pieces(change: &Change, quote: &Quote) -> Vec<Piece> {
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
pub(super) fn quotes(pieces: &[Piece]) -> Vec<Quote> {
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

/// Pieces packed into chunks that each fit the row budget, a test too long
/// for one shown in consecutive parts. `None` when a single line cannot fit.
pub(super) fn fitted(
    all: &[Piece],
    fits: &dyn Fn(usize, &[Piece]) -> bool,
) -> Option<Vec<Vec<Piece>>> {
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    for piece in all {
        let mut trial = current.clone();
        trial.push(piece.clone());
        if fits(groups.len(), &trial) {
            current = trial;
            continue;
        }
        if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
        let mut rest = piece.clone();
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
    if !current.is_empty() {
        groups.push(current);
    }
    Some(groups)
}

/// Whole pieces in at most `max_chunks` groups of about equal length.
pub(super) fn balanced(all: &[Piece], max_chunks: usize) -> Vec<Vec<Piece>> {
    let length = |piece: &Piece| piece.end + 1 - piece.start;
    let total: usize = all.iter().map(length).sum();
    let target = total.div_ceil(max_chunks.max(1)).max(1);
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    let mut lines = 0;
    for piece in all {
        current.push(piece.clone());
        lines += length(piece);
        if lines >= target {
            groups.push(std::mem::take(&mut current));
            lines = 0;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}
