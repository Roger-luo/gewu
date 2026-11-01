use std::collections::HashSet;

use super::prose;
use crate::ValidatedReview;

/// Names and paths a reference may resolve to.
///
/// Built from the manifest alone, it knows only what the diff touches. An
/// interface that can read the reviewed tree should add its words and paths,
/// because prose rightly names unchanged code the diff depends on.
#[derive(Clone, Debug, Default)]
pub struct SymbolIndex {
    words: HashSet<String>,
    paths: HashSet<String>,
    /// The diff's own text, where a path the code builds at run time — a
    /// lock file, a record directory — appears as a literal.
    literals: String,
}

impl SymbolIndex {
    /// Every identifier in either side of the diff, and every changed path.
    /// Removed code counts: prose may name what a change deletes.
    #[must_use]
    pub fn from_review(review: &ValidatedReview) -> Self {
        let changes = &review.input().changes;
        let literals = changes.iter().map(|change| change.patch.as_str()).collect();
        Self {
            literals,
            ..Self::default()
        }
        .with_words(
            changes
                .iter()
                .flat_map(|change| prose::identifiers(&change.patch))
                .map(str::to_owned),
        )
        .with_paths(changes.iter().map(|change| change.path.clone()))
    }

    #[must_use]
    pub fn with_words(mut self, words: impl IntoIterator<Item = String>) -> Self {
        self.words.extend(words);
        self
    }

    /// Text where a run-time path may appear literally, such as files of the
    /// reviewed tree the diff leaves unchanged.
    #[must_use]
    pub fn with_literals(mut self, texts: impl IntoIterator<Item = String>) -> Self {
        for text in texts {
            self.literals.push('\n');
            self.literals.push_str(&text);
        }
        self
    }

    #[must_use]
    pub fn with_paths(mut self, paths: impl IntoIterator<Item = String>) -> Self {
        self.paths.extend(paths);
        self
    }

    pub(super) fn has_word(&self, word: &str) -> bool {
        self.words.contains(word)
    }

    /// A path resolves whole, as a directory, as a trailing component — so
    /// prose may write `spec.rs` for `crates/gewu-review/src/spec.rs` — or as
    /// a literal in the diff. An upper-case placeholder segment, as in
    /// `calls/NNNN`, stands for whatever the code fills in, so only the text
    /// before it must resolve.
    pub(super) fn has_path(&self, path: &str) -> bool {
        let known = |path: &str| {
            self.literals.contains(path)
                || self.paths.iter().any(|known| {
                    known == path
                        || known.starts_with(&format!("{path}/"))
                        || known.ends_with(&format!("/{path}"))
                })
        };
        if known(path) {
            return true;
        }
        let placeholder = path.split('/').position(|segment| {
            segment.len() >= 2
                && segment.chars().any(|c| c.is_ascii_uppercase())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        });
        placeholder.is_some_and(|index| {
            let prefix: Vec<_> = path.split('/').take(index).collect();
            !prefix.is_empty() && known(&format!("{}/", prefix.join("/")))
        })
    }
}
