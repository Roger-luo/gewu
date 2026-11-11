//! Splitting test code into pieces, one test each, and packing them into
//! chunks of whole pieces: within the row budget where they fit, and
//! otherwise in at most the item's chunk cap. A test is never cut in two.

use super::scan;
use crate::{Change, Quote};

/// What a piece of test code is, for its chunk's title and prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    /// It defines a test.
    Test,
    /// It adds code the tests use, such as a helper or a fixture.
    Helper,
    /// It changes lines of code already there, such as an existing test.
    Edit,
}

/// One test, or the code between tests, within one change.
#[derive(Clone)]
pub(super) struct Piece {
    pub(super) change_id: String,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) role: Role,
    pub(super) name: Option<String>,
    pub(super) doc: Option<String>,
    /// The module's own doc comment (`//!`), where this piece carries it.
    pub(super) module_doc: Option<String>,
    /// Every line is added, so the piece reads as code written whole.
    pub(super) added: bool,
}

impl Piece {
    pub(super) fn lines(&self) -> usize {
        self.end + 1 - self.start
    }
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
            while start > last && scan::leads_test(lines[start - 1]) {
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
            let added = span.iter().all(|line| line.starts_with('+'));
            let test = span.iter().position(|line| scan::starts_test(line));
            // Found by QA of the #276 walkthrough: helpers such as `propose`
            // were titled as tests, and edits inside existing tests were
            // counted as whole tests "shown as written".
            let (role, name) = match test {
                Some(at) => (Role::Test, scan::test_name(&span[at..])),
                None => match scan::defined(span) {
                    Some(name) => (Role::Helper, Some(name)),
                    None if added => (Role::Helper, None),
                    None => (Role::Edit, None),
                },
            };
            Piece {
                change_id: change.id.clone(),
                start: quote.start + pair[0],
                end: quote.start + pair[1] - 1,
                role,
                name,
                doc: test.and_then(|_| scan::doc(span)),
                module_doc: scan::module_doc(span),
                added,
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

/// Whole pieces packed into chunks that fit the row budget; a piece too long
/// for one chunk gets one of its own, which scrolls. Found by QA of the #276
/// walkthrough: cutting a test mid-loop left two chunks with one title.
pub(super) fn whole(all: &[Piece], fits: &dyn Fn(usize, &[Piece]) -> bool) -> Vec<Vec<Piece>> {
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    for piece in all {
        let mut trial = current.clone();
        trial.push(piece.clone());
        if current.is_empty() || fits(groups.len(), &trial) {
            current = trial;
        } else {
            groups.push(std::mem::replace(&mut current, vec![piece.clone()]));
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Whole pieces in at most `max_chunks` groups of about equal length.
pub(super) fn balanced(all: &[Piece], max_chunks: usize) -> Vec<Vec<Piece>> {
    let total: usize = all.iter().map(Piece::lines).sum();
    let target = total.div_ceil(max_chunks.max(1)).max(1);
    let mut groups: Vec<Vec<Piece>> = Vec::new();
    let mut current: Vec<Piece> = Vec::new();
    let mut lines = 0;
    for piece in all {
        current.push(piece.clone());
        lines += piece.lines();
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
