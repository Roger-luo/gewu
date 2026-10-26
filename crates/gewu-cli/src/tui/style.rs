use crossterm::style::{
    Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, Default)]
pub struct TextStyle {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    pub style: TextStyle,
}

fn spans_match(left: TextStyle, right: TextStyle) -> bool {
    format!("{left:?}") == format!("{right:?}")
}

/// Columns a run of characters occupies, which is not its character count once
/// wide glyphs are involved.
fn display_width(characters: &[(char, TextStyle)]) -> usize {
    characters
        .iter()
        .map(|(character, _)| character.width().unwrap_or(0))
        .sum()
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn push(&mut self, text: impl Into<String>, style: TextStyle) {
        let text = text.into();
        if !text.is_empty() {
            self.spans.push(Span { text, style });
        }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.spans
            .iter()
            .flat_map(|span| span.text.chars())
            .map(|character| character.width().unwrap_or(0))
            .sum()
    }

    #[must_use]
    pub fn clipped(&self, width: usize) -> Self {
        let mut remaining = width;
        let mut line = Self::default();
        for span in &self.spans {
            let mut text = String::new();
            for character in span.text.chars() {
                let character_width = character.width().unwrap_or(0);
                if character_width > remaining {
                    break;
                }
                remaining = remaining.saturating_sub(character_width);
                text.push(character);
            }
            line.push(text, span.style);
            if remaining == 0 {
                break;
            }
        }
        line
    }

    /// Splits this line to fit a width, breaking at spaces and carrying the
    /// line's leading gutter or indent onto each continuation.
    #[must_use]
    pub fn wrapped(&self, width: usize) -> Vec<Self> {
        if width == 0 || self.width() <= width {
            return vec![self.clone()];
        }
        let characters: Vec<_> = self
            .spans
            .iter()
            .flat_map(|span| span.text.chars().map(|character| (character, span.style)))
            .collect();
        let indent = Self::continuation_indent(&characters, width);
        let mut lines = Vec::new();
        let mut current: Vec<(char, TextStyle)> = Vec::new();
        let mut current_width = 0;
        let mut breakpoint = None;
        for entry in characters {
            let entry_width = entry.0.width().unwrap_or(0);
            if current_width + entry_width > width {
                match breakpoint.filter(|at| *at > current.len().min(indent.len())) {
                    Some(at) => {
                        let rest: Vec<_> = current.split_off(at);
                        lines.push(Self::from_characters(&current));
                        current = indent.clone();
                        current.extend(rest.iter().skip_while(|(c, _)| *c == ' ').copied());
                    }
                    // A single unbreakable token: cut it rather than lose it.
                    None => {
                        lines.push(Self::from_characters(&current));
                        current = indent.clone();
                    }
                }
                current_width = display_width(&current);
                breakpoint = None;
            }
            if entry.0 == ' ' && current.len() > indent.len() {
                breakpoint = Some(current.len());
            }
            current_width += entry_width;
            current.push(entry);
        }
        if current.iter().any(|(character, _)| *character != ' ') || lines.is_empty() {
            lines.push(Self::from_characters(&current));
        }
        lines
    }

    /// What a continuation line starts with. Structural prefixes — the source
    /// gutter and indentation — are reproduced, while markers that name the
    /// line, such as a line number or a bullet, become blanks so wrapped text
    /// stays aligned under the text it continues.
    fn continuation_indent(
        characters: &[(char, TextStyle)],
        width: usize,
    ) -> Vec<(char, TextStyle)> {
        let mut indent = Vec::new();
        let mut rest = characters.iter();
        let mut pending = Vec::new();
        for (character, style) in rest.by_ref() {
            if character.is_whitespace() || *character == '▏' {
                indent.append(&mut pending);
                indent.push((*character, *style));
            } else if character.is_ascii_digit() || matches!(character, '•' | '-' | '*' | '.' | ')')
            {
                pending.push((' ', *style));
            } else {
                break;
            }
        }
        // A runaway prefix would leave no room for content.
        if display_width(&indent) + 8 > width {
            return Vec::new();
        }
        indent
    }

    fn from_characters(characters: &[(char, TextStyle)]) -> Self {
        let mut line = Self::default();
        for (character, style) in characters {
            match line.spans.last_mut() {
                Some(span) if spans_match(span.style, *style) => span.text.push(*character),
                _ => line.push(character.to_string(), *style),
            }
        }
        line
    }

    pub fn pad_to(&mut self, width: usize) {
        let padding = width.saturating_sub(self.width());
        self.push(" ".repeat(padding), TextStyle::default());
    }

    fn ansi(&self) -> String {
        let mut output = String::new();
        for span in &self.spans {
            if let Some(color) = span.style.foreground {
                output.push_str(&SetForegroundColor(color).to_string());
            }
            if let Some(color) = span.style.background {
                output.push_str(&SetBackgroundColor(color).to_string());
            }
            if span.style.bold {
                output.push_str(&SetAttribute(Attribute::Bold).to_string());
            }
            if span.style.italic {
                output.push_str(&SetAttribute(Attribute::Italic).to_string());
            }
            if span.style.underlined {
                output.push_str(&SetAttribute(Attribute::Underlined).to_string());
            }
            output.push_str(&span.text);
            output.push_str(&ResetColor.to_string());
            output.push_str(&SetAttribute(Attribute::Reset).to_string());
        }
        output
    }
}

#[derive(Clone, Debug, Default)]
pub struct Document {
    pub lines: Vec<Line>,
}

impl Document {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![Line::default()],
        }
    }

    pub fn push(&mut self, text: impl Into<String>, style: TextStyle) {
        if self.lines.is_empty() {
            self.lines.push(Line::default());
        }
        for (index, part) in text.into().split('\n').enumerate() {
            if index > 0 {
                self.newline();
            }
            self.lines
                .last_mut()
                .expect("document has a line")
                .push(part, style);
        }
    }

    pub fn newline(&mut self) {
        self.lines.push(Line::default());
    }

    pub fn append(&mut self, other: Self) {
        if other.lines.is_empty() {
            return;
        }
        let mut lines = other.lines.into_iter();
        if let Some(first) = lines.next() {
            self.lines
                .last_mut()
                .expect("document has a line")
                .spans
                .extend(first.spans);
        }
        self.lines.extend(lines);
    }

    /// Prefixes every line with a marker, framing quoted source material so it
    /// reads apart from the review's own prose.
    #[must_use]
    pub fn gutter(self, marker: &str, style: TextStyle) -> Self {
        Self {
            lines: self
                .lines
                .into_iter()
                .map(|line| {
                    let mut gutter = Line::default();
                    gutter.push(marker, style);
                    gutter.spans.extend(line.spans);
                    gutter
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn trim_trailing(mut self) -> Self {
        while self.lines.last().is_some_and(|line| line.width() == 0) {
            self.lines.pop();
        }
        self
    }

    /// Reflows every line to a width. Terminal output clips, so prose that is
    /// not wrapped is prose the reviewer never sees.
    #[must_use]
    pub fn wrapped(self, width: usize) -> Self {
        Self {
            lines: self
                .lines
                .iter()
                .flat_map(|line| line.wrapped(width))
                .collect(),
        }
    }

    #[must_use]
    pub fn columns(left: Self, right: Self, width: usize) -> Self {
        let left_width = width.saturating_mul(2) / 3;
        let right_width = width.saturating_sub(left_width + 3);
        let (left, right) = (left.wrapped(left_width), right.wrapped(right_width));
        let count = left.lines.len().max(right.lines.len());
        let mut document = Self { lines: Vec::new() };
        for index in 0..count {
            let mut line = left
                .lines
                .get(index)
                .cloned()
                .unwrap_or_default()
                .clipped(left_width);
            line.pad_to(left_width);
            line.push(
                " │ ",
                TextStyle {
                    foreground: Some(Color::DarkGrey),
                    ..TextStyle::default()
                },
            );
            let rhs = right
                .lines
                .get(index)
                .cloned()
                .unwrap_or_default()
                .clipped(right_width);
            line.spans.extend(rhs.spans);
            document.lines.push(line);
        }
        document
    }

    #[must_use]
    pub fn ansi_window(&self, width: usize, height: usize, offset: usize) -> String {
        self.lines
            .iter()
            .skip(offset)
            .take(height)
            .map(|line| line.clipped(width).ansi())
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, TextStyle};

    #[test]
    fn wraps_prose_at_word_boundaries_instead_of_dropping_it() {
        let mut document = Document::new();
        document.push(
            "the reviewer must be able to read every word of this explanation",
            TextStyle::default(),
        );

        let wrapped = document.wrapped(20);

        let text: Vec<String> = wrapped
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>()
            })
            .collect();
        assert!(wrapped.lines.len() > 1);
        assert!(
            wrapped.lines.iter().all(|line| line.width() <= 20),
            "{text:?}"
        );
        assert_eq!(
            text.join(" "),
            "the reviewer must be able to read every word of this explanation"
        );
    }

    #[test]
    fn carries_the_source_gutter_onto_continuation_lines() {
        let mut document = Document::new();
        document.push("▏ ", TextStyle::default());
        document.push(
            "quoted material that runs past the edge",
            TextStyle::default(),
        );

        let wrapped = document.wrapped(20);

        assert!(wrapped.lines.len() > 1);
        assert!(
            wrapped.lines.iter().all(|line| {
                line.spans
                    .first()
                    .is_some_and(|span| span.text.starts_with('▏'))
            }),
            "{:?}",
            wrapped.lines
        );
    }

    #[test]
    fn wraps_on_display_width_not_character_count() {
        let mut document = Document::new();
        // Each of these occupies two columns, so twelve of them overflow a
        // twenty-column terminal even though they are only twelve characters.
        document.push("来自大学来自大学来自大学", TextStyle::default());

        let wrapped = document.wrapped(20);

        assert!(wrapped.lines.len() > 1, "{:?}", wrapped.lines);
        assert!(wrapped.lines.iter().all(|line| line.width() <= 20));
        let text: String = wrapped
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(text, "来自大学来自大学来自大学", "no characters were lost");
    }

    #[test]
    fn renders_a_scrolled_window() {
        let mut document = Document::new();
        document.push("first", TextStyle::default());
        document.newline();
        document.push("second", TextStyle::default());
        document.newline();
        document.push("third", TextStyle::default());

        let visible = document.ansi_window(80, 1, 1);

        assert!(visible.contains("second"));
        assert!(!visible.contains("first"));
        assert!(!visible.contains("third"));
    }

    #[test]
    fn reflowing_narrow_then_wide_preserves_long_paths_and_unicode() {
        let mut document = Document::new();
        let text = "src/a/very/long/component/name/reviewer.rs · café → 東京";
        document.push(text, TextStyle::default());

        let narrow = document.clone().wrapped(18);
        let restored = document.wrapped(110);
        let restored_text: String = restored
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(narrow.lines.len() > 1);
        assert_eq!(restored.lines.len(), 1);
        assert_eq!(restored_text, text);
    }
}
