use std::collections::HashSet;

use gewu_review::LineAnchor;

use super::ReviewWorkspace;
use crate::{AnchorRef, AppError};

impl ReviewWorkspace {
    /// Fill path, side, and patch hash, and check every named line on that
    /// side appears in this chunk's quotes for the change. A quote is the block
    /// the reviewer reads, so a comment on a whole block names its lines, and a
    /// span that reaches past the block is rejected.
    pub(super) fn resolve_anchor(
        &self,
        chunk_id: &str,
        reference: &AnchorRef,
    ) -> Result<LineAnchor, AppError> {
        if reference.start_line == 0
            || reference.end_line == 0
            || reference.end_line < reference.start_line
        {
            return Err(AppError::AnchorOutsideQuote);
        }
        let change = self
            .review
            .input()
            .changes
            .iter()
            .find(|change| change.id == reference.change_id)
            .ok_or_else(|| AppError::UnknownChange(reference.change_id.clone()))?;
        let chunk = self
            .review
            .spec()
            .chunks
            .iter()
            .find(|chunk| chunk.id == chunk_id)
            .ok_or_else(|| AppError::UnknownChunk(chunk_id.into()))?;
        let side = reference.side;
        let quoted: HashSet<u32> = chunk
            .quotes()
            .filter(|quote| quote.change_id == change.id)
            .flat_map(|quote| {
                (quote.start..=quote.end).filter_map(|body_line| change.line_at(body_line, side))
            })
            .collect();
        if (reference.start_line..=reference.end_line).any(|line| !quoted.contains(&line)) {
            return Err(AppError::AnchorOutsideQuote);
        }
        Ok(LineAnchor {
            change_id: change.id.clone(),
            path: change.path.clone(),
            side: side.as_str().into(),
            start_line: reference.start_line,
            end_line: reference.end_line,
            patch_sha256: change.patch_sha256(),
        })
    }
}
