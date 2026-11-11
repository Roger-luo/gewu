use std::collections::{BTreeMap, HashSet};

use gewu_review::ReviewInput;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BlockView, ChunkView};

/// One file of the review's whole scope, not only of the generated steps.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileView {
    pub path: String,
    pub added: usize,
    pub removed: usize,
    /// Whether a generated step quotes this file yet.
    pub shown: bool,
}

impl FileView {
    /// Every changed file in the input, in path order.
    pub(crate) fn collect(input: &ReviewInput, chunks: &[ChunkView]) -> Vec<Self> {
        let shown: HashSet<&str> = chunks
            .iter()
            .flat_map(|chunk| &chunk.blocks)
            .filter_map(|block| match block {
                BlockView::Changes { quotes, .. } => Some(quotes),
                BlockView::Narrative { .. } | BlockView::Explainer(_) => None,
            })
            .flatten()
            .map(|quote| quote.path.as_str())
            .collect();
        let mut files = BTreeMap::<&str, Self>::new();
        for change in &input.changes {
            let (added, removed) = change.line_counts();
            let file = files.entry(&change.path).or_insert_with(|| Self {
                path: change.path.clone(),
                added: 0,
                removed: 0,
                shown: shown.contains(change.path.as_str()),
            });
            file.added += added;
            file.removed += removed;
        }
        files.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use gewu_review::{Change, ReviewInput, SourceIdentity};

    use super::FileView;

    /// Found by judging a real run: an incomplete batch listed only the files
    /// its generated steps quoted, so the overview hid half the change.
    #[test]
    fn every_changed_file_is_listed_before_a_step_shows_it() {
        let input = ReviewInput::new(
            SourceIdentity::new("base", "head", b"diff"),
            vec![
                Change::new(
                    "README.md",
                    (1, 1, 1, 2),
                    "@@ -1 +1,2 @@\n-old\n+new\n+more\n",
                ),
                Change::new("docs/guide.md", (1, 0, 1, 1), "@@ -1,0 +1 @@\n+guide\n"),
            ],
        );

        let files = FileView::collect(&input, &[]);

        assert_eq!(
            files,
            vec![
                FileView {
                    path: "README.md".into(),
                    added: 2,
                    removed: 1,
                    shown: false,
                },
                FileView {
                    path: "docs/guide.md".into(),
                    added: 1,
                    removed: 0,
                    shown: false,
                },
            ]
        );
    }
}
