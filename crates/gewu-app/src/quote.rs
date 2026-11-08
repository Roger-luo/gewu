use std::path::Path;

use gewu_review::{Change, Quote};

use crate::snapshot::{QuoteLine, QuoteLineKind, QuoteView};

impl QuoteView {
    /// Resolve a quote against its change: body lines in the span with kinds,
    /// old/new numbers from the hunk header, and whether the hunk continues
    /// outside the span.
    pub(crate) fn resolve(quote: &Quote, change: &Change) -> Self {
        let body = change.body(quote.start, quote.end);
        let coords = LineCoords::from(change);
        let lines = (quote.start..=quote.end)
            .zip(body)
            .filter_map(|(body_line, text)| coords.line(body_line, text))
            .collect();
        Self {
            change_id: change.id.clone(),
            path: change.path.clone(),
            start: quote.start,
            end: quote.end,
            language: language_of(&change.path),
            more_above: quote.start > 1,
            more_below: quote.end < change.body_lines(),
            lines,
        }
    }
}

/// Old and new file line numbers for each 1-based hunk-body line.
struct LineCoords {
    old: Vec<Option<u32>>,
    new: Vec<Option<u32>>,
    kinds: Vec<QuoteLineKind>,
}

impl LineCoords {
    fn from(change: &Change) -> Self {
        let mut old_next = change.old_start;
        let mut new_next = change.new_start;
        let mut old = Vec::new();
        let mut new = Vec::new();
        let mut kinds = Vec::new();
        for text in change
            .patch
            .lines()
            .skip(usize::from(change.patch.starts_with("@@ ")))
        {
            match text.chars().next() {
                Some('-') => {
                    old.push(Some(old_next));
                    new.push(None);
                    kinds.push(QuoteLineKind::Removed);
                    old_next = old_next.saturating_add(1);
                }
                Some('+') => {
                    old.push(None);
                    new.push(Some(new_next));
                    kinds.push(QuoteLineKind::Added);
                    new_next = new_next.saturating_add(1);
                }
                Some(' ') => {
                    old.push(Some(old_next));
                    new.push(Some(new_next));
                    kinds.push(QuoteLineKind::Context);
                    old_next = old_next.saturating_add(1);
                    new_next = new_next.saturating_add(1);
                }
                _ => {
                    old.push(None);
                    new.push(None);
                    kinds.push(QuoteLineKind::Context);
                }
            }
        }
        Self { old, new, kinds }
    }

    fn line(&self, body_line: usize, text: &str) -> Option<QuoteLine> {
        let index = body_line.checked_sub(1)?;
        Some(QuoteLine {
            kind: *self.kinds.get(index)?,
            old_line: self.old.get(index).copied().flatten(),
            new_line: self.new.get(index).copied().flatten(),
            text: strip_marker(text),
        })
    }
}

fn strip_marker(text: &str) -> String {
    text.strip_prefix(['+', '-', ' ']).unwrap_or(text).into()
}

fn language_of(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "py" => "python",
            "go" => "go",
            "md" | "mdx" => "markdown",
            "toml" => "toml",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "sh" | "bash" | "zsh" => "shell",
            "css" => "css",
            "html" | "htm" => "html",
            "c" => "c",
            "h" | "hh" | "hpp" => "cpp",
            "cc" | "cpp" | "cxx" => "cpp",
            "java" => "java",
            "kt" => "kotlin",
            "swift" => "swift",
            "rb" => "ruby",
            "txt" => "text",
            other => other,
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use gewu_review::{Change, Quote};

    use super::QuoteView;
    use crate::snapshot::QuoteLineKind;

    #[test]
    fn resolves_kinds_and_line_numbers_from_the_hunk() {
        let change = Change::new(
            "src/lib.rs",
            (10, 2, 20, 3),
            "@@ -10,2 +20,3 @@\n context\n-removed\n+added\n+last\n",
        );
        let view = QuoteView::resolve(
            &Quote::whole(change.id.clone(), change.body_lines()),
            &change,
        );

        assert_eq!(view.language.as_deref(), Some("rust"));
        assert!(!view.more_above);
        assert!(!view.more_below);
        assert_eq!(view.lines.len(), 4);
        assert_eq!(view.lines[0].kind, QuoteLineKind::Context);
        assert_eq!(view.lines[0].old_line, Some(10));
        assert_eq!(view.lines[0].new_line, Some(20));
        assert_eq!(view.lines[0].text, "context");
        assert_eq!(view.lines[1].kind, QuoteLineKind::Removed);
        assert_eq!(view.lines[1].old_line, Some(11));
        assert_eq!(view.lines[1].new_line, None);
        assert_eq!(view.lines[2].kind, QuoteLineKind::Added);
        assert_eq!(view.lines[2].new_line, Some(21));
        assert_eq!(view.lines[3].new_line, Some(22));
    }

    #[test]
    fn more_flags_mark_omitted_hunk_lines() {
        let change = Change::new(
            "a.py",
            (1, 4, 1, 4),
            "@@ -1,4 +1,4 @@\n one\n-two\n+deux\n three\n four\n",
        );
        let view = QuoteView::resolve(
            &Quote {
                change_id: change.id.clone(),
                start: 2,
                end: 3,
            },
            &change,
        );

        assert!(view.more_above);
        assert!(view.more_below);
        assert_eq!(view.language.as_deref(), Some("python"));
        assert_eq!(view.lines.len(), 2);
    }

    #[test]
    fn empty_body_lines_lose_their_marker() {
        let change = Change::new("a.md", (1, 2, 1, 2), "@@ -1,2 +1,2 @@\n \n-\n+\n");
        let view = QuoteView::resolve(
            &Quote::whole(change.id.clone(), change.body_lines()),
            &change,
        );

        assert!(view.lines.iter().all(|line| line.text.is_empty()));
    }
}
