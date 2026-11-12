//! Loops a chunk quotes but does not draw. A reader follows a loop that
//! transforms data by running it in their head; a stepper over one example
//! does that for them. Found by blind proofreads of ppvm #233: every chunk
//! that failed `showing` quoted an added `for` or `while` loop and carried no
//! explainer, while the writer drew steppers elsewhere in the walkthrough.

use std::collections::HashMap;

use crate::{Change, ChunkBlock, ChunkKind, ReviewChunk};

/// How an added line starts when it opens a loop, in the languages Gewu
/// reviews most: Rust, Python, and C-like code.
const LOOP_STARTS: &[&str] = &["for ", "while ", "loop {", "for(", "while("];

impl ReviewChunk {
    /// The first added loop line this chunk quotes, when it draws no
    /// explainer and is not a test.
    #[must_use]
    pub fn undrawn_loop<'a>(&self, changes: &HashMap<&String, &'a Change>) -> Option<&'a str> {
        if self.kind == ChunkKind::Test
            || self
                .content
                .iter()
                .any(|block| matches!(block, ChunkBlock::Explainer(_)))
        {
            return None;
        }
        self.content.iter().find_map(|block| {
            let ChunkBlock::Changes { quotes, .. } = block else {
                return None;
            };
            quotes.iter().find_map(|quote| {
                let change = changes.get(&quote.change_id)?;
                change
                    .body(quote.start, quote.end)
                    .into_iter()
                    .filter_map(|line| line.strip_prefix('+'))
                    .map(str::trim)
                    .find(|code| LOOP_STARTS.iter().any(|start| code.starts_with(start)))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{Change, ReviewChunk};

    fn change(patch: &str) -> Change {
        Change::new("src/rows.rs", (1, 3, 1, 3), patch)
    }

    fn chunk(change: &Change, kind: &str, explainer: bool) -> ReviewChunk {
        let mut content = vec![serde_json::json!({
            "type": "changes", "presentation": "added_code",
            "quotes": [{ "change_id": change.id, "start": 1, "end": 3 }]
        })];
        if explainer {
            content.push(serde_json::json!({
                "type": "explainer", "kind": "claim", "claim": "c", "how": "h"
            }));
        }
        serde_json::from_value(serde_json::json!({
            "id": "k", "title": "t", "summary": "s", "kind": kind, "content": content
        }))
        .expect("chunk")
    }

    #[test]
    fn a_quoted_added_loop_without_an_explainer_is_found() {
        let change =
            change("@@ -1,0 +1,3 @@\n+fn sweep(t: &mut T) {\n+    for column in 0..n {\n+}\n");
        let changes = HashMap::from([(&change.id, &change)]);
        assert_eq!(
            chunk(&change, "concept", false).undrawn_loop(&changes),
            Some("for column in 0..n {")
        );
        assert_eq!(chunk(&change, "concept", true).undrawn_loop(&changes), None);
        assert_eq!(chunk(&change, "test", false).undrawn_loop(&changes), None);
    }

    #[test]
    fn a_removed_or_unchanged_loop_is_not() {
        let change =
            change("@@ -1,3 +1,2 @@\n fn sweep() {\n-    for x in xs {}\n     while ok {}\n");
        let changes = HashMap::from([(&change.id, &change)]);
        assert_eq!(
            chunk(&change, "concept", false).undrawn_loop(&changes),
            None
        );
    }
}
