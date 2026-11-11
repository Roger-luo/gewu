//! What one quoted source line costs a reviewer to read.
//!
//! The rates come from the rubric's `[reading]` table ([`Rates`]); the
//! lexicon below only says what kind of line a line is.

use std::collections::HashSet;

use crate::load::Rates;

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "if", "else", "match", "for", "while", "loop", "return", "impl",
    "struct", "enum", "trait", "type", "const", "static", "use", "mod", "self", "Self", "true",
    "false", "def", "class", "import", "from", "in", "as", "where", "async", "await", "move",
    "ref", "break", "continue", "elif", "None", "Some", "Ok", "Err",
];
const BRANCHES: &[&str] = &[
    "if", "else", "match", "for", "while", "loop", "case", "switch", "catch", "except", "elif",
    "return", "break", "continue",
];
const DECLARES: &[&str] = &[
    "let", "fn", "struct", "enum", "trait", "type", "const", "static", "def", "class", "var",
    "function",
];

/// How a quoted line stands in the diff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Side {
    Added,
    Removed,
    Context,
}

impl Side {
    pub(super) fn of(line: &str) -> (Self, &str) {
        match line.as_bytes().first() {
            Some(b'+') => (Self::Added, &line[1..]),
            Some(b'-') => (Self::Removed, &line[1..]),
            Some(b' ') => (Self::Context, &line[1..]),
            _ => (Self::Context, line),
        }
    }
}

/// Prices lines in reading order, remembering shapes already read.
pub(super) struct LineCost<'a> {
    rates: &'a Rates,
    seen: HashSet<String>,
}

impl<'a> LineCost<'a> {
    pub(super) fn new(rates: &'a Rates) -> Self {
        Self {
            rates,
            seen: HashSet::new(),
        }
    }

    /// Seconds to read `text` (without its diff marker).
    pub(super) fn of(&mut self, side: Side, text: &str) -> f64 {
        let rates = self.rates;
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.chars().all(|c| "}])>;,".contains(c)) {
            return rates.trivial_line;
        }
        if let Some(words) = comment(trimmed) {
            let rate = if side == Side::Context { 0.3 } else { 1.0 };
            return (words as f64 * rates.per_word() * rate).max(rates.trivial_line);
        }
        if is_import(trimmed) {
            return rates.import_line;
        }
        let shape = shape(trimmed);
        let repeat = shape.len() >= 6 && !self.seen.insert(shape);
        match side {
            Side::Context => rates.context_line,
            Side::Removed if repeat => rates.repeated_line,
            Side::Removed => rates.removed_line,
            Side::Added if repeat => rates.repeated_line,
            Side::Added => (rates.changed_line + surcharge(text, rates)).min(rates.line_ceiling),
        }
    }
}

/// Words in a comment line, or `None` when the line is code.
fn comment(trimmed: &str) -> Option<usize> {
    let body = ["///", "//!", "//", "/*", "*/", "* ", "--", "\"\"\""]
        .iter()
        .find_map(|marker| trimmed.strip_prefix(marker))
        .or_else(|| {
            // `#[derive]` and `#!` are attributes; `# text` is a comment.
            trimmed
                .strip_prefix('#')
                .filter(|rest| !rest.starts_with(['[', '!']))
        })?;
    Some(body.split_whitespace().count())
}

fn is_import(trimmed: &str) -> bool {
    [
        "use ",
        "pub use ",
        "import ",
        "from ",
        "#include",
        "mod ",
        "pub mod ",
        "extern crate",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
        || trimmed.starts_with("#[")
}

/// Branching, new names, and nesting each make a line slower to follow.
fn surcharge(text: &str, rates: &Rates) -> f64 {
    let words: Vec<_> = words(text).collect();
    let branch = words.iter().any(|word| BRANCHES.contains(word))
        || text.contains("=>")
        || text.contains("&&")
        || text.contains("||")
        || text.trim_end().ends_with('?')
        || text.contains("?;")
        || text.contains("?.");
    let declares = words.iter().any(|word| DECLARES.contains(word));
    let indent: usize = text
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    let levels = (indent / 4).saturating_sub(3);
    f64::from(u8::from(branch)) * rates.branch
        + f64::from(u8::from(declares)) * rates.new_name
        + levels as f64 * rates.nesting_level
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}

/// The line with names, numbers, and strings blanked, so `a.push(1)` and
/// `b.push(2)` read as one shape.
fn shape(trimmed: &str) -> String {
    let mut out = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            for next in chars.by_ref() {
                if next == '"' {
                    break;
                }
            }
            out.push_str("\"\"");
        } else if c.is_alphanumeric() || c == '_' {
            let mut word = String::from(c);
            while let Some(&next) = chars.peek() {
                if !(next.is_alphanumeric() || next == '_') {
                    break;
                }
                word.push(next);
                chars.next();
            }
            if c.is_ascii_digit() {
                out.push('0');
            } else if KEYWORDS.contains(&word.as_str()) {
                out.push_str(&word);
            } else {
                out.push('x');
            }
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}
