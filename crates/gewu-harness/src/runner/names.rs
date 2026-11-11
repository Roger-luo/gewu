//! Code names a draft's prose cites, checked against the code: a function
//! the walkthrough names as a caller must exist at the base or the head.

use gewu_review::{ChunkBlock, ReviewChunk};

use super::{HarnessRunner, Run};

impl HarnessRunner {
    /// A reminder naming code the draft's prose cites that exists at neither
    /// revision. Skipped for working-tree reviews, whose head is no commit,
    /// and whenever Git cannot answer.
    pub(super) fn name_reminder(&self, run: &Run, chunks: &[ReviewChunk]) -> String {
        if run.state.origin.working_tree {
            return String::new();
        }
        let repository = gewu_git::Repository::new(&self.project);
        let source = &run.input.source;
        let unknown = unknown_names(chunks, |name| {
            [&source.head, &source.base]
                .iter()
                .any(|revision| repository.tree_contains(revision, name).unwrap_or(true))
        });
        if unknown.is_empty() {
            return String::new();
        }
        let listed: Vec<String> = unknown
            .iter()
            .map(|(chunk, name)| format!("- `{chunk}` names `{name}`"))
            .collect();
        format!(
            "\nNames no file contains at the base or the head; check each against the code \
             and use the real name, or drop it:\n\n{}\n",
            listed.join("\n")
        )
    }
}

/// The snake_case names in backticks in `markdown`: `foo_bar`, `foo_bar()`,
/// and the last segment of `module::foo_bar`. Types and single words are
/// left out; they are too often prose.
fn cited(markdown: &str) -> impl Iterator<Item = &str> {
    markdown.split('`').skip(1).step_by(2).filter_map(|span| {
        let name = span.trim().trim_end_matches("()");
        let name = name.rsplit("::").next().unwrap_or(name);
        let snake = name.contains('_')
            && name.starts_with(|c: char| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        snake.then_some(name)
    })
}

/// Names the prose of `chunks` cites that `known` does not find, each once,
/// with the chunk that first cites it.
///
/// Found by QA of Cursor's run E of #276: a section named
/// `leakage_orbit_rep_with_prune` and `build_mf_cols_orbit_rep` as callers,
/// and neither exists at the base or the head.
pub(super) fn unknown_names(
    chunks: &[ReviewChunk],
    mut known: impl FnMut(&str) -> bool,
) -> Vec<(String, String)> {
    let mut checked = Vec::new();
    let mut unknown = Vec::new();
    for chunk in chunks {
        let mut prose = vec![chunk.summary.as_str()];
        for block in &chunk.content {
            match block {
                ChunkBlock::Narrative { markdown } => prose.push(markdown),
                ChunkBlock::Explainer(explainer) => prose.extend(explainer.prose()),
                _ => {}
            }
        }
        for name in prose.into_iter().flat_map(cited) {
            if checked.iter().any(|seen| seen == name) {
                continue;
            }
            checked.push(name.to_owned());
            if !known(name) {
                unknown.push((chunk.id.clone(), name.to_owned()));
            }
        }
    }
    unknown
}

#[cfg(test)]
mod tests {
    use gewu_review::{ChunkBlock, ChunkKind, ReviewChunk};

    use super::unknown_names;

    fn chunk(markdown: &str) -> ReviewChunk {
        ReviewChunk {
            id: "chunk-1".into(),
            title: "Title".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Concept,
            content: vec![ChunkBlock::Narrative {
                markdown: markdown.into(),
            }],
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
        }
    }

    /// Found by QA of Cursor's run E of #276: invented caller names.
    #[test]
    fn names_the_code_does_not_contain_are_reported_once() {
        let chunks = [chunk(
            "Called by `leakage_orbit_rep()` and `basis::build_mf_cols_orbit_rep`, \
             then `build_mf_cols_orbit_rep` again; `Word`, `x`, and `GeneralWord` are skipped.",
        )];
        let unknown = unknown_names(&chunks, |name| name == "leakage_orbit_rep");
        assert_eq!(
            unknown,
            vec![("chunk-1".to_owned(), "build_mf_cols_orbit_rep".to_owned())]
        );
    }
}
