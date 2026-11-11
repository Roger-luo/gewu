//! A chunk as plain text, the way a decider reads it: what the reviewer sees,
//! in order, without the renderer's chrome.

use std::{collections::HashMap, fmt::Write};

use crate::{Change, ChunkBlock, ChunkContext, ReviewChunk};

/// The longest state sent, in characters (about 8k tokens). A chunk longer
/// than this is overloaded on its reading time alone; the cut keeps the
/// request cheap and says it was cut.
pub const MAX_STATE_CHARS: usize = 32_000;

/// How much of the state the earlier chunks may take, in characters. The
/// nearest are kept: they are what the reader has freshest.
pub const MAX_EARLIER_CHARS: usize = 8_000;

/// The titles and summaries of the chunks before this one, then the chunk's
/// title, summary, prose, quoted lines, explainers, and context pane, in
/// reading order. Without what came before, every name an earlier chunk
/// introduced looks unexplained.
#[must_use]
pub fn chunk_state(
    chunk: &ReviewChunk,
    changes: &HashMap<&String, &Change>,
    earlier: &[ReviewChunk],
) -> String {
    let mut out = String::new();
    if !earlier.is_empty() {
        let mut lines = Vec::new();
        let mut used = 0;
        for before in earlier.iter().rev() {
            let line = format!("- {}: {}\n", before.title, before.summary);
            if used + line.len() > MAX_EARLIER_CHARS {
                lines.push(format!(
                    "- … {} earlier chunks not listed\n",
                    earlier.len() - lines.len()
                ));
                break;
            }
            used += line.len();
            lines.push(line);
        }
        out.push_str("# Earlier chunks in this walkthrough\n\n");
        out.extend(lines.iter().rev().map(String::as_str));
        out.push('\n');
    }
    let _ = write!(out, "# This chunk: {}\n\n{}\n", chunk.title, chunk.summary);
    for block in &chunk.content {
        match block {
            ChunkBlock::Narrative { markdown } => {
                let _ = write!(out, "\n{markdown}\n");
            }
            ChunkBlock::Explainer(explainer) => {
                let json = serde_json::to_string(explainer).unwrap_or_default();
                let _ = write!(out, "\n[explainer] {json}\n");
            }
            ChunkBlock::Changes { quotes, .. } => {
                for quote in quotes {
                    let Some(change) = changes.get(&quote.change_id) else {
                        continue;
                    };
                    let _ = write!(
                        out,
                        "\n```{} (hunk lines {}-{})\n{}\n```\n",
                        change.path,
                        quote.start,
                        quote.end,
                        change.body(quote.start, quote.end).join("\n")
                    );
                }
            }
        }
    }
    if let Some(confidence) = &chunk.confidence {
        let _ = write!(out, "\nConfidence: {confidence}\n");
    }
    if let Some(context) = &chunk.context {
        let (a, b, c) = match context {
            ChunkContext::Concept {
                definition,
                example,
                relation,
            } => (definition, example, relation),
            ChunkContext::Refactor {
                before,
                after,
                reason,
            } => (before, after, reason),
        };
        let _ = write!(out, "\n[context pane]\n{a}\n{b}\n{c}\n");
    }
    if out.len() > MAX_STATE_CHARS {
        let mut end = MAX_STATE_CHARS;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n[cut: the chunk continues]\n");
    }
    out
}
