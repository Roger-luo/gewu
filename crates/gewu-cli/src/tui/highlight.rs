use std::{path::Path, sync::OnceLock};

use crossterm::style::Color;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use super::style::{Document, Line, TextStyle};

const ADDED_BACKGROUND: Color = Color::Rgb {
    r: 20,
    g: 70,
    b: 35,
};

pub struct CodeHighlighter {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl CodeHighlighter {
    #[must_use]
    pub fn global() -> &'static Self {
        static HIGHLIGHTER: OnceLock<CodeHighlighter> = OnceLock::new();
        HIGHLIGHTER.get_or_init(|| {
            let themes = ThemeSet::load_defaults();
            Self {
                syntaxes: SyntaxSet::load_defaults_newlines(),
                theme: themes.themes["base16-ocean.dark"].clone(),
            }
        })
    }

    #[must_use]
    pub fn code(&self, language: &str, code: &str) -> Document {
        let syntax = self
            .syntaxes
            .find_syntax_by_token(language)
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        self.highlight(syntax, code)
    }

    #[must_use]
    pub fn source(&self, path: &str, code: &str) -> Document {
        let syntax = self
            .syntaxes
            .find_syntax_for_file(Path::new(path))
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        self.highlight(syntax, code)
    }

    #[must_use]
    pub fn diff(&self, path: &str, patch: &str) -> Document {
        let syntax = self
            .syntaxes
            .find_syntax_for_file(Path::new(path))
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut document = Document { lines: Vec::new() };
        for text in patch.lines() {
            if text.starts_with("@@") {
                let mut line = Line::default();
                line.push(
                    text,
                    TextStyle {
                        foreground: Some(Color::Cyan),
                        bold: true,
                        ..TextStyle::default()
                    },
                );
                document.lines.push(line);
                continue;
            }
            let (marker, code, marker_color, added) = match text.chars().next() {
                Some('+') => ("+", &text[1..], Color::Green, true),
                Some('-') => ("-", &text[1..], Color::Red, false),
                Some(' ') => (" ", &text[1..], Color::DarkGrey, false),
                _ => ("", text, Color::DarkGrey, false),
            };
            let mut line = Line::default();
            line.push(
                marker,
                TextStyle {
                    foreground: Some(marker_color),
                    bold: marker != " ",
                    ..TextStyle::default()
                },
            );
            line.spans.extend(self.line(&mut highlighter, code).spans);
            if added {
                for span in &mut line.spans {
                    span.style.background = Some(ADDED_BACKGROUND);
                }
            }
            document.lines.push(line);
        }
        document
    }

    fn highlight(&self, syntax: &SyntaxReference, code: &str) -> Document {
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut document = Document { lines: Vec::new() };
        for line in code.lines() {
            document.lines.push(self.line(&mut highlighter, line));
        }
        if document.lines.is_empty() {
            document.lines.push(Line::default());
        }
        document
    }

    fn line(&self, highlighter: &mut HighlightLines<'_>, text: &str) -> Line {
        let mut line = Line::default();
        match highlighter.highlight_line(text, &self.syntaxes) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    line.push(text, Self::style(style));
                }
            }
            Err(_) => line.push(text, TextStyle::default()),
        }
        line
    }

    fn style(style: Style) -> TextStyle {
        TextStyle {
            foreground: Some(Color::Rgb {
                r: style.foreground.r,
                g: style.foreground.g,
                b: style.foreground.b,
            }),
            background: None,
            bold: style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD),
            italic: style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC),
            underlined: style
                .font_style
                .contains(syntect::highlighting::FontStyle::UNDERLINE),
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::style::Color;

    use super::{ADDED_BACKGROUND, CodeHighlighter};

    #[test]
    fn highlights_diff_markers_and_rust_tokens() {
        let document = CodeHighlighter::global().diff(
            "src/lib.rs",
            "@@ -1 +1 @@\n-pub fn old() {}\n+pub fn new() {}\n",
        );
        let spans: Vec<_> = document.lines.iter().flat_map(|line| &line.spans).collect();

        assert!(spans.iter().any(|span| {
            span.text == "+"
                && span.style.foreground == Some(Color::Green)
                && span.style.background == Some(ADDED_BACKGROUND)
        }));
        assert!(spans.iter().any(|span| {
            span.text.contains("pub") && matches!(span.style.foreground, Some(Color::Rgb { .. }))
        }));
    }

    #[test]
    fn highlights_added_source_without_diff_markers() {
        let document = CodeHighlighter::global().source("src/lib.rs", "pub fn added() {}");
        let spans: Vec<_> = document.lines.iter().flat_map(|line| &line.spans).collect();

        assert!(!spans.iter().any(|span| span.text == "+"));
        assert!(spans.iter().any(|span| {
            span.text.contains("pub") && matches!(span.style.foreground, Some(Color::Rgb { .. }))
        }));
    }
}
