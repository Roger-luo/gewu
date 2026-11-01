//! Just enough Markdown reading to compare prose against source text.

/// Words too common in prose and code to show that a sentence is about a
/// particular quote. Short words are excluded by length before this list.
const COMMON: [&str; 73] = [
    "about", "above", "after", "also", "async", "await", "because", "been", "before", "being",
    "below", "between", "both", "break", "change", "changes", "code", "const", "continue", "crate",
    "does", "each", "else", "enum", "every", "false", "file", "files", "first", "from", "have",
    "here", "impl", "into", "just", "last", "line", "lines", "loop", "make", "match", "more",
    "must", "next", "none", "only", "other", "over", "return", "same", "self", "should", "some",
    "static", "struct", "such", "super", "than", "that", "their", "them", "then", "there", "they",
    "this", "trait", "true", "type", "what", "when", "where", "while", "with",
];

/// Extensions that make a dotted code span a file path rather than a field
/// or method chain.
const EXTENSIONS: [&str; 14] = [
    "rs", "md", "toml", "json", "jsonl", "py", "lock", "yml", "yaml", "txt", "sh", "js", "ts",
    "html",
];

/// Identifier-shaped tokens, in order, with everything else as separators.
pub(super) fn identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|token| token.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'))
}

/// Whether a token is specific enough that sharing it ties prose to code.
pub(super) fn distinctive(token: &str) -> bool {
    token.len() >= 4 && !COMMON.contains(&token.to_ascii_lowercase().as_str())
}

/// Inline code spans outside fenced blocks. Fenced blocks are illustrations,
/// not references to the corpus, so nothing inside them is checked.
pub(super) fn code_spans(markdown: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut fenced = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut rest = line;
        while let Some(open) = rest.find('`') {
            let ticks = rest[open..].len() - rest[open..].trim_start_matches('`').len();
            let fence = &rest[open..open + ticks];
            let body = &rest[open + ticks..];
            let Some(close) = body.find(fence) else { break };
            spans.push(body[..close].trim());
            rest = &body[close + ticks..];
        }
    }
    spans
}

/// What a code span claims exists in the corpus.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Reference<'a> {
    Path(&'a str),
    /// `Quote::covers` or `spec.chunks`, split into its segments.
    Symbol(Vec<&'a str>),
}

impl<'a> Reference<'a> {
    /// Classifies a span, or returns `None` for commands, expressions,
    /// literals, and flags, which name nothing that must resolve.
    pub(super) fn parse(span: &'a str) -> Option<Self> {
        let span = span
            .trim_start_matches('&')
            .trim_end_matches("()")
            .trim_end_matches('!');
        if span.is_empty() || span.contains(char::is_whitespace) {
            return None;
        }
        let path_like = span.contains('/')
            || span.rsplit_once('.').is_some_and(|(stem, extension)| {
                !stem.is_empty() && EXTENSIONS.contains(&extension)
            });
        if path_like {
            let valid = span
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));
            let path = span.trim_end_matches('/');
            return (valid && !path.is_empty()).then_some(Self::Path(path));
        }
        let segments: Vec<_> = span.split("::").flat_map(|part| part.split('.')).collect();
        let identifier = |segment: &&str| {
            segment.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        segments
            .iter()
            .all(identifier)
            .then_some(Self::Symbol(segments))
    }

    /// The name a reader would look up: the last segment of a symbol path.
    pub(super) fn name(&self) -> Option<&'a str> {
        match self {
            Self::Path(_) => None,
            Self::Symbol(segments) => segments.last().copied(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_skip_fences_and_handle_double_ticks() {
        let markdown = "Uses `Quote` and ``a`b``.\n```rust\nlet `x` = 1;\n```\nThen `covers`.";
        assert_eq!(code_spans(markdown), ["Quote", "a`b", "covers"]);
    }

    #[test]
    fn references_distinguish_paths_symbols_and_commands() {
        assert_eq!(
            Reference::parse("Quote::covers()"),
            Some(Reference::Symbol(vec!["Quote", "covers"]))
        );
        assert_eq!(
            Reference::parse("src/spec.rs"),
            Some(Reference::Path("src/spec.rs"))
        );
        assert_eq!(
            Reference::parse("Cargo.lock"),
            Some(Reference::Path("Cargo.lock"))
        );
        assert_eq!(
            Reference::parse("self.coverage"),
            Some(Reference::Symbol(vec!["self", "coverage"]))
        );
        assert_eq!(Reference::parse("/"), None);
        assert_eq!(Reference::parse("mise run check"), None);
        assert_eq!(Reference::parse("--partial"), None);
        assert_eq!(Reference::parse("0..=3"), None);
    }
}
