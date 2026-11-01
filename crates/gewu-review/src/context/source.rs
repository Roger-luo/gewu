use serde::{Deserialize, Serialize};

use crate::{Quote, SourceIdentity};

use super::{ContextError, ContextReader, body::BodyIndex};

/// Offset into the UTF-8 encoding of the quote's body lines joined by `\n`.
/// It may fall within a very long source line, but never within a character.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceCursor {
    pub source: SourceIdentity,
    pub quote: Quote,
    pub offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourcePage {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub quote: Quote,
    pub path: String,
    pub offset: usize,
    /// Original hunk-body line containing the first character of this page.
    pub first_body_line: usize,
    /// A continuation may start within that line. This is a byte offset.
    pub first_line_byte: usize,
    pub text: String,
    pub next: Option<SourceCursor>,
}

impl ContextReader<'_> {
    pub fn source(
        &self,
        quote: &Quote,
        cursor: Option<&SourceCursor>,
    ) -> Result<SourcePage, ContextError> {
        let (change, index) = self
            .bodies
            .get(quote.change_id.as_str())
            .ok_or(ContextError::InvalidQuote)?;
        if quote.start == 0 || quote.end < quote.start || quote.end > change.body_lines() {
            return Err(ContextError::InvalidQuote);
        }
        let body = index.get_or_init(|| BodyIndex::new(&change.patch));
        let (start, end) = body
            .range(quote.start, quote.end)
            .ok_or(ContextError::InvalidQuote)?;
        let length = end - start;
        let offset = match cursor {
            Some(cursor)
                if cursor.source == self.input.source
                    && cursor.quote == *quote
                    && cursor.offset < length
                    && body.is_char_boundary(start + cursor.offset) =>
            {
                cursor.offset
            }
            Some(_) => return Err(ContextError::InvalidCursor),
            None => 0,
        };
        let (line, byte) = body.location(start + offset);
        let mut page = SourcePage {
            schema_version: 1,
            source: self.input.source.clone(),
            quote: quote.clone(),
            path: change.path.clone(),
            offset,
            first_body_line: line + 1,
            first_line_byte: byte,
            text: body.window(start + offset, end, self.max_bytes),
            next: None,
        };
        if length == 0 {
            return self.bounded(page);
        }
        loop {
            if page.text.is_empty() {
                return Err(self.too_small());
            }
            let next = offset + page.text.len();
            page.next = (next < length).then(|| SourceCursor {
                source: self.input.source.clone(),
                quote: quote.clone(),
                offset: next,
            });
            let size = Self::size(&page)?;
            if size <= self.max_bytes {
                return Ok(page);
            }
            // Escaping can make a byte several JSON bytes. Do not jump to zero
            // just because the first candidate's excess exceeds its raw size.
            let reduction = (size - self.max_bytes).min(page.text.len().div_ceil(2));
            let mut keep = page.text.len() - reduction;
            while !page.text.is_char_boundary(keep) {
                keep -= 1;
            }
            page.text.truncate(keep);
        }
    }
}
