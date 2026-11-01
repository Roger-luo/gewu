use serde::{Deserialize, Serialize};

use crate::{Quote, SourceIdentity};

use super::{ContextError, ContextReader, body::BodyIndex};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchCursor {
    pub source: SourceIdentity,
    pub query: String,
    pub change: usize,
    pub line: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub quote: Quote,
    pub body_lines: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchPage {
    pub source: SourceIdentity,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub next: Option<SearchCursor>,
}

impl ContextReader<'_> {
    /// Literal source discovery, not a semantic dependency or absence proof.
    /// Matching lines are references only; source text uses the bounded reader.
    pub fn search(
        &self,
        query: &str,
        cursor: Option<&SearchCursor>,
    ) -> Result<SearchPage, ContextError> {
        if query.is_empty() || query.len() > 256 {
            return Err(ContextError::InvalidQuery);
        }
        let (start_change, start_line) = match cursor {
            Some(cursor)
                if cursor.source == self.input.source
                    && cursor.query == query
                    && self
                        .input
                        .changes
                        .get(cursor.change)
                        .is_some_and(|change| cursor.line < change.body_lines()) =>
            {
                (cursor.change, cursor.line)
            }
            Some(_) => return Err(ContextError::InvalidCursor),
            None => (0, 0),
        };
        let mut page = SearchPage {
            source: self.input.source.clone(),
            query: query.into(),
            matches: vec![],
            next: None,
        };
        for (index, change) in self.input.changes.iter().enumerate().skip(start_change) {
            let first = if index == start_change { start_line } else { 0 };
            let (_, body) = self
                .bodies
                .get(change.id.as_str())
                .ok_or(ContextError::InvalidQuote)?;
            let body = body.get_or_init(|| BodyIndex::new(&change.patch));
            for (line, text) in body.lines_from(first) {
                if !text.contains(query) {
                    continue;
                }
                let previous = page.next.clone();
                page.matches.push(SearchMatch {
                    path: change.path.clone(),
                    quote: Quote {
                        change_id: change.id.clone(),
                        start: line + 1,
                        end: line + 1,
                    },
                    body_lines: change.body_lines(),
                });
                let (next_change, next_line) = if line + 1 < change.body_lines() {
                    (index, line + 1)
                } else {
                    (index + 1, 0)
                };
                page.next = (next_change < self.input.changes.len()).then(|| SearchCursor {
                    source: self.input.source.clone(),
                    query: query.into(),
                    change: next_change,
                    line: next_line,
                });
                if Self::size(&page)? > self.max_bytes {
                    page.matches.pop();
                    page.next = previous;
                    if page.matches.is_empty() {
                        return Err(self.too_small());
                    }
                    return self.bounded(page);
                }
            }
        }
        page.next = None;
        self.bounded(page)
    }
}
