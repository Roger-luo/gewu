//! Sentences that mostly repeat earlier text, and references to chunk titles
//! that do not exist.
//!
//! Across self-review runs the judged minimality stayed near 0.8 because chunk
//! prose restated the overview, the title, or itself, and prompt rules alone did
//! not move it. These checks give the author the same signal while drafting.
//! They are lexical, so they are advisory: a judge decides what is redundant.

use std::collections::HashSet;

use serde::Serialize;

use super::{ChunkProse, prose};
use crate::{Change, ReviewChunk, ReviewSpec};

/// Share of a sentence's distinctive words found in one earlier text before
/// the sentence counts as an echo of it. Calibrated against 342 judged
/// sentences from self-review: at 0.6 the check flags 32, of which 44% the
/// judge called redundant, and finds 20% of all redundancy. Paraphrase defeats
/// word overlap, so this is a lead for the judge, never a gate.
const ECHO: f64 = 0.6;
/// Sentences with fewer distinctive words say too little to compare.
const MIN_WORDS: usize = 4;

#[derive(Clone, Debug, Serialize)]
pub struct Echo {
    pub chunk_id: String,
    pub sentence: String,
    /// What it repeats: `overview`, `title`, or an earlier chunk's id.
    pub repeats: String,
}

/// A quoted or emphasized phrase that reads like a chunk title but names none.
#[derive(Clone, Debug, Serialize)]
pub struct DanglingTitle {
    pub chunk_id: String,
    pub title: String,
}

/// Echoes in `chunks`, compared against the overview and everything before
/// each sentence, including `earlier` chunks already in the walkthrough.
pub fn echoes(spec: &ReviewSpec, earlier: &[ReviewChunk], chunks: &[ReviewChunk]) -> Vec<Echo> {
    let mut seen: Vec<(String, HashSet<String>)> = std::iter::once(&spec.overview.intent)
        .chain(&spec.overview.changes)
        .map(|text| ("overview".to_owned(), words(text)))
        .collect();
    for chunk in earlier {
        seen.extend(
            sentences(chunk)
                .into_iter()
                .map(|s| (chunk.id.clone(), words(&s))),
        );
    }
    let mut found = Vec::new();
    for chunk in chunks {
        let mut local = seen.clone();
        local.push(("title".to_owned(), words(&chunk.title)));
        for sentence in sentences(chunk) {
            let own = words(&sentence);
            if own.len() >= MIN_WORDS
                && let Some((source, _)) = local.iter().find(|(_, other)| {
                    own.intersection(other).count() as f64 >= ECHO * own.len() as f64
                })
            {
                found.push(Echo {
                    chunk_id: chunk.id.clone(),
                    sentence: sentence.clone(),
                    repeats: source.clone(),
                });
            }
            local.push((chunk.id.clone(), own));
        }
        seen = local
            .into_iter()
            .filter(|(source, _)| source != "title")
            .collect();
    }
    found
}

/// Phrases in `*…*` or `“…”` of three or more words, starting upper-case,
/// that match no chunk title and appear nowhere in the diff.
pub fn dangling_titles(
    titles: &[&str],
    chunks: &[ReviewChunk],
    changes: &[Change],
) -> Vec<DanglingTitle> {
    let known: HashSet<String> = titles
        .iter()
        .map(|title| title.trim().to_lowercase())
        .collect();
    let mut found = Vec::new();
    for chunk in chunks {
        let text = ChunkProse::new(chunk).text();
        for phrase in quoted(&text) {
            let lower = phrase.to_lowercase();
            let titled = phrase.starts_with(char::is_uppercase)
                && phrase.split_whitespace().count() >= 3
                && !phrase.contains('`');
            if titled
                && !known.contains(&lower)
                && !changes
                    .iter()
                    .any(|change| change.patch.to_lowercase().contains(&lower))
            {
                found.push(DanglingTitle {
                    chunk_id: chunk.id.clone(),
                    title: phrase,
                });
            }
        }
    }
    found
}

fn quoted(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (open, close) in [("“", "”"), ("*", "*")] {
        let mut rest = text;
        while let Some(start) = rest.find(open) {
            let after = &rest[start + open.len()..];
            let Some(end) = after.find(close) else { break };
            let phrase = after[..end].trim().trim_end_matches(['.', ',']);
            if !phrase.is_empty() && !phrase.contains('\n') {
                found.push(phrase.to_owned());
            }
            rest = &after[end + close.len()..];
        }
    }
    found
}

/// The summary and narrative sentences of a chunk, outside fenced examples.
fn sentences(chunk: &ReviewChunk) -> Vec<String> {
    let mut parts = vec![chunk.summary.clone()];
    parts.extend(chunk.content.iter().filter_map(|block| match block {
        crate::ChunkBlock::Narrative { markdown } => Some(markdown.clone()),
        crate::ChunkBlock::Changes { .. } => None,
    }));
    let mut out = Vec::new();
    for part in parts {
        let mut fenced = false;
        let mut text = String::new();
        for line in part.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if !fenced {
                text.push(' ');
                text.push_str(line.trim());
            }
        }
        let characters: Vec<char> = text.chars().collect();
        let mut current = String::new();
        for (index, character) in characters.iter().enumerate() {
            current.push(*character);
            let boundary = matches!(character, '.' | '?' | '!')
                && characters
                    .get(index + 1)
                    .is_none_or(|next| next.is_whitespace());
            if boundary {
                out.push(current.trim().to_owned());
                current.clear();
            }
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_owned());
        }
    }
    out.retain(|sentence| !sentence.is_empty());
    out
}

fn words(text: &str) -> HashSet<String> {
    prose::identifiers(text)
        .filter(|word| prose::distinctive(word))
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::quoted;

    #[test]
    fn quoted_phrases_come_from_curly_quotes_and_emphasis() {
        assert_eq!(
            quoted("See *Summary, footer, and help wording* and “The wait gate”. Not *x*."),
            ["The wait gate", "Summary, footer, and help wording", "x"]
        );
    }
}
