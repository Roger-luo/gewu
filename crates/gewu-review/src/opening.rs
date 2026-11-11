//! How a chunk's prose opens. A first sentence that only points at lines
//! ("Lines 85–92 split …", "In `shortlist.rs`, lines 12–40 …") tells the
//! reviewer where they are but not what part of the change this is or why it
//! is read now. On 120 proofread chunks from ten walkthroughs, 29 of the 36
//! this finds scored below 2 on `connection`, against 32 of the other 84
//! (docs/proofread-prescreen.md).

use crate::{ChunkBlock, ReviewChunk};

/// Words a locator opening may start with.
const LEADS: &[&str] = &["line", "lines", "in", "at", "on"];
/// How far into the sentence a line range may sit and still be the opening.
const WINDOW: usize = 6;

impl ReviewChunk {
    /// The first sentence of the chunk's prose, when it opens by pointing at
    /// a line range rather than saying where the chunk fits.
    #[must_use]
    pub fn locator_opening(&self) -> Option<&str> {
        let prose = self.content.iter().find_map(|block| match block {
            ChunkBlock::Narrative { markdown } => Some(markdown.trim()),
            _ => None,
        })?;
        let sentence = first_sentence(prose);
        let words: Vec<String> = sentence
            .split_whitespace()
            .take(WINDOW + 1)
            .map(|word| {
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .collect();
        if !LEADS.contains(&words.first()?.as_str()) {
            return None;
        }
        let ranged = words.windows(2).any(|pair| {
            matches!(pair[0].as_str(), "line" | "lines")
                && pair[1].starts_with(|c: char| c.is_ascii_digit())
        });
        ranged.then_some(sentence)
    }
}

/// Up to the first full stop followed by a space, or the whole text.
fn first_sentence(prose: &str) -> &str {
    let line = prose.lines().next().unwrap_or(prose);
    line.find(". ").map_or(line, |end| &line[..=end])
}

#[cfg(test)]
mod tests {
    use crate::{ChunkBlock, ReviewChunk};

    fn opening(markdown: &str) -> Option<String> {
        let chunk: ReviewChunk = serde_json::from_value(serde_json::json!({
            "id": "c", "title": "t", "summary": "s", "kind": "concept",
            "content": [{ "type": "narrative", "markdown": markdown }]
        }))
        .expect("chunk");
        assert!(matches!(chunk.content[0], ChunkBlock::Narrative { .. }));
        chunk.locator_opening().map(str::to_owned)
    }

    #[test]
    fn a_line_range_opening_is_found() {
        for text in [
            "Lines 85–92 split `buffer.numeric` into rates and keys. Then more.",
            "In `shortlist.rs`, lines 12–40 build the heap.",
            "At line 7 the loop starts.",
            "In crates/ppvm/src/step.rs lines 3-9 reserve the tail.",
        ] {
            assert!(opening(text).is_some(), "{text}");
        }
        assert_eq!(
            opening("Lines 1–3 do this. And that.").as_deref(),
            Some("Lines 1–3 do this.")
        );
    }

    #[test]
    fn an_opening_that_places_the_chunk_is_not() {
        for text in [
            "With the heap built, `update` keeps it ordered (lines 85–92).",
            "In the previous chunk the buffer was split; here lines 3-9 use it.",
            "The runner acts on the verdict that `next_step` returns.",
            "Line endings are normalised before hashing.",
        ] {
            assert!(opening(text).is_none(), "{text}");
        }
    }
}
