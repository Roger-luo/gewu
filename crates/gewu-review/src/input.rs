use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{DEFAULT_MAX_CHUNK_LINES, SCHEMA_VERSION, Side};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceIdentity {
    pub base: String,
    pub head: String,
    pub diff_sha256: String,
}

impl SourceIdentity {
    #[must_use]
    pub fn new(base: impl Into<String>, head: impl Into<String>, diff: &[u8]) -> Self {
        Self {
            base: base.into(),
            head: head.into(),
            diff_sha256: hex::encode(Sha256::digest(diff)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Change {
    pub id: String,
    pub path: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// Lines a quote may name, so an agent never has to derive it. This is not
    /// `new_lines`: that counts the new side only, and a hunk with removals has
    /// more body lines than new ones.
    #[serde(default)]
    pub body_lines: usize,
    pub patch: String,
}

impl Change {
    /// Identity is the file and the edited lines themselves — not the commit
    /// they came from, and not where in the file they sit. A hunk that nothing
    /// touched keeps its id across a re-diff even when edits above it shift it
    /// down, so review work already done against it stays valid.
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        ranges: (u32, u32, u32, u32),
        patch: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let patch = patch.into();
        let (old_start, old_lines, new_start, new_lines) = ranges;
        let digest = hex::encode(Sha256::digest(Self::material(&path, &patch).as_bytes()));
        let body_lines = patch
            .lines()
            .skip(usize::from(patch.starts_with("@@ ")))
            .count()
            .max(1);
        Self {
            id: format!("change-{}", &digest[..16]),
            path,
            old_start,
            old_lines,
            new_start,
            new_lines,
            body_lines,
            patch,
        }
    }

    /// The hunk header carries line numbers, so it is excluded; a metadata
    /// patch has no header and is hashed whole.
    fn material(path: &str, patch: &str) -> String {
        let body = match patch.strip_prefix("@@ ") {
            Some(rest) => rest.split_once('\n').map_or("", |(_, body)| body),
            None => patch,
        };
        format!("{path}\0{body}")
    }

    /// Distinguishes hunks that are byte-identical within one file.
    fn at_occurrence(mut self, occurrence: usize) -> Self {
        if occurrence > 0 {
            self.id = format!("{}-{occurrence}", self.id);
        }
        self
    }

    #[must_use]
    pub fn patch_sha256(&self) -> String {
        hex::encode(Sha256::digest(self.patch.as_bytes()))
    }

    #[must_use]
    pub fn body_lines(&self) -> usize {
        self.body_lines
    }

    /// Added and removed body lines, as a diff stat counts them.
    #[must_use]
    pub fn line_counts(&self) -> (usize, usize) {
        self.patch
            .lines()
            .skip(usize::from(self.patch.starts_with("@@ ")))
            .fold((0, 0), |(added, removed), text| {
                match text.as_bytes().first() {
                    Some(b'+') => (added + 1, removed),
                    Some(b'-') => (added, removed + 1),
                    _ => (added, removed),
                }
            })
    }

    /// The body lines covering a span of the new file, 1-based and inclusive.
    ///
    /// A reviewer anchors a request to the line number they can see, which is
    /// the new side's. Removed lines carry no new-side number, so they are
    /// counted through rather than numbered.
    #[must_use]
    pub fn new_side(&self, start: u32, end: u32) -> Vec<&str> {
        let mut line = self.new_start;
        let mut found = Vec::new();
        for text in self
            .patch
            .lines()
            .skip(usize::from(self.patch.starts_with("@@ ")))
        {
            if text.starts_with('-') {
                continue;
            }
            if line >= start && line <= end {
                found.push(text);
            }
            line = line.saturating_add(1);
        }
        found
    }

    /// Whether coverage must show this body line. Blank lines carry nothing
    /// to review, and neither does the unchanged context Git puts at a hunk's
    /// edges: it belongs to the code around the change, and requiring it made
    /// authors quote the next, unrelated test's attribute (found by
    /// self-review). Context between two edits stays, because it is read with
    /// them.
    #[must_use]
    pub fn needs_review(&self, line: usize) -> bool {
        let lines: Vec<_> = self.body(1, self.body_lines());
        let Some(text) = lines.get(line.wrapping_sub(1)) else {
            return true;
        };
        if text.get(1..).unwrap_or_default().trim().is_empty() {
            return false;
        }
        if !self.patch.starts_with("@@ ") || !text.starts_with(' ') {
            return true;
        }
        let edited = |text: &&str| text.starts_with(['+', '-']);
        let first = lines.iter().position(edited);
        let last = lines.iter().rposition(edited);
        matches!((first, last), (Some(first), Some(last)) if line > first && line <= last + 1)
    }

    /// The body lines a quote names, 1-based and inclusive.
    #[must_use]
    pub fn body(&self, start: usize, end: usize) -> Vec<&str> {
        self.patch
            .lines()
            .skip(usize::from(self.patch.starts_with("@@ ")))
            .skip(start.saturating_sub(1))
            .take(end.saturating_sub(start.saturating_sub(1)))
            .collect()
    }

    /// The new-side file line represented by one 1-based hunk-body line.
    /// Removed and metadata lines have no new-side location.
    #[must_use]
    pub fn new_line_at(&self, body_line: usize) -> Option<u32> {
        self.line_at(body_line, Side::New)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewInput {
    pub schema_version: u32,
    pub source: SourceIdentity,
    /// Rendered rows a chunk may occupy before it must justify itself. Raise it
    /// only when a reviewer has asked for larger chunks.
    #[serde(default = "default_max_chunk_lines")]
    pub max_chunk_lines: usize,
    /// Estimated reading seconds a chunk may take before it must justify
    /// itself. The desktop app sets it, because it scrolls; when set, it
    /// replaces the row budget ([`crate::ChunkBudget`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chunk_seconds: Option<u32>,
    pub changes: Vec<Change>,
}

impl ReviewInput {
    #[must_use]
    pub fn new(source: SourceIdentity, changes: Vec<Change>) -> Self {
        Self::sized(source, changes, DEFAULT_MAX_CHUNK_LINES)
    }

    #[must_use]
    pub fn sized(source: SourceIdentity, changes: Vec<Change>, max_chunk_lines: usize) -> Self {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let changes = changes
            .into_iter()
            .map(|change| {
                let occurrence = seen.entry(change.id.clone()).or_default();
                let numbered = change.at_occurrence(*occurrence);
                *occurrence += 1;
                numbered
            })
            .collect();
        Self {
            schema_version: SCHEMA_VERSION,
            source,
            max_chunk_lines,
            max_chunk_seconds: None,
            changes,
        }
    }

    /// Changes this manifest shares with an earlier one. Ids are addressed by
    /// content, so a surviving id is the same edited lines in the same file,
    /// wherever they now sit.
    #[must_use]
    pub fn unchanged_since(&self, previous: &Self) -> HashSet<&String> {
        let before: HashSet<_> = previous.changes.iter().map(|change| &change.id).collect();
        self.changes
            .iter()
            .map(|change| &change.id)
            .filter(|id| before.contains(*id))
            .collect()
    }

    #[must_use]
    pub fn with_max_chunk_lines(mut self, lines: usize) -> Self {
        self.max_chunk_lines = lines.max(1);
        self
    }

    /// Holds chunks to `seconds` of estimated reading instead of rows.
    #[must_use]
    pub fn with_max_chunk_seconds(mut self, seconds: Option<u32>) -> Self {
        self.max_chunk_seconds = seconds.map(|seconds| seconds.max(1));
        self
    }
}

fn default_max_chunk_lines() -> usize {
    DEFAULT_MAX_CHUNK_LINES
}

#[cfg(test)]
mod tests {
    use super::Change;

    #[test]
    fn maps_hunk_body_lines_to_the_new_side() {
        let change = Change::new(
            "src/lib.rs",
            (10, 2, 20, 3),
            "@@ -10,2 +20,3 @@\n context\n-removed\n+added\n+last\n",
        );

        assert_eq!(change.new_line_at(1), Some(20));
        assert_eq!(change.new_line_at(2), None);
        assert_eq!(change.new_line_at(3), Some(21));
        assert_eq!(change.new_line_at(4), Some(22));
        assert_eq!(change.new_line_at(5), None);
    }

    /// Found by self-review: coverage made an author quote the `#[test]`
    /// line of the next, unchanged test, which Git includes as context.
    #[test]
    fn edge_context_needs_no_review_but_context_between_edits_does() {
        let change = Change::new(
            "a.rs",
            (1, 6, 1, 6),
            "@@ -1,6 +1,6 @@\n before\n-old\n+new\n between\n+more\n\n #[test]\n",
        );
        let needed: Vec<_> = (1..=change.body_lines())
            .filter(|line| change.needs_review(*line))
            .collect();
        assert_eq!(needed, [2, 3, 4, 5]);
    }
}
