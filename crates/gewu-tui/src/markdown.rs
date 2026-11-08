use crossterm::style::Color;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{
    highlight::CodeHighlighter,
    render_support::muted,
    style::{Document, TextStyle},
};

pub struct Markdown;

impl Markdown {
    #[must_use]
    pub fn render(source: &str) -> Document {
        let options =
            Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
        Renderer::new().render(Parser::new_ext(source, options))
    }
}

struct Renderer {
    document: Document,
    style: TextStyle,
    styles: Vec<TextStyle>,
    code_block: Option<(String, String)>,
    /// Next number for each open ordered list; `None` marks a bullet list.
    lists: Vec<Option<u64>>,
}

impl Renderer {
    fn new() -> Self {
        Self {
            document: Document::new(),
            style: TextStyle::default(),
            styles: Vec::new(),
            code_block: None,
            lists: Vec::new(),
        }
    }

    fn render<'a>(mut self, parser: Parser<'a>) -> Document {
        for event in parser {
            if let Some((_, code)) = &mut self.code_block {
                match event {
                    Event::Text(text) => code.push_str(&text),
                    Event::End(TagEnd::CodeBlock) => self.finish_code_block(),
                    _ => {}
                }
                continue;
            }
            match event {
                Event::Start(tag) => self.start(tag),
                Event::End(tag) => self.end(tag),
                Event::Text(text) => self.document.push(text.into_string(), self.style),
                Event::Code(code) => {
                    let mut style = self.style;
                    style.foreground = Some(Color::Yellow);
                    self.document.push(code.into_string(), style);
                }
                Event::SoftBreak => self.document.push(" ", self.style),
                Event::HardBreak => self.document.newline(),
                Event::Rule => {
                    self.document.push("─".repeat(40), TextStyle::default());
                    self.document.newline();
                }
                Event::TaskListMarker(checked) => {
                    self.document
                        .push(if checked { "[x] " } else { "[ ] " }, self.style);
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    self.document.push(html.into_string(), self.style);
                }
                _ => {}
            }
        }
        self.document
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { .. } => self.with_style(|style| {
                style.foreground = Some(Color::Cyan);
                style.bold = true;
            }),
            Tag::BlockQuote(_) => {
                self.document.push(
                    "│ ",
                    TextStyle {
                        foreground: Some(Color::DarkGrey),
                        ..TextStyle::default()
                    },
                );
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(language) => language.into_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code_block = Some((language, String::new()));
            }
            Tag::List(first) => self.lists.push(first),
            Tag::Item => {
                let marker = match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}. ");
                        *number += 1;
                        marker
                    }
                    _ => "• ".to_owned(),
                };
                self.document.push(marker, self.style);
            }
            Tag::Emphasis => self.with_style(|style| style.italic = true),
            Tag::Strong => self.with_style(|style| style.bold = true),
            Tag::Strikethrough => self.with_style(|style| {
                style.foreground = Some(Color::DarkGrey);
            }),
            Tag::Link { .. } => self.with_style(|style| {
                style.foreground = Some(Color::Blue);
                style.underlined = true;
            }),
            Tag::TableCell
                if self
                    .document
                    .lines
                    .last()
                    .is_some_and(|line| line.width() > 0) =>
            {
                self.document.push(" │ ", self.style);
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::BlockQuote(_) => {
                self.document.newline();
                self.document.newline();
            }
            TagEnd::Heading(_) => {
                self.style = self.styles.pop().unwrap_or_default();
                self.document.newline();
                self.document.newline();
            }
            TagEnd::Item | TagEnd::TableRow => self.document.newline(),
            TagEnd::List(_) => {
                self.lists.pop();
                self.document.newline();
            }
            TagEnd::Table => self.document.newline(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.style = self.styles.pop().unwrap_or_default();
            }
            TagEnd::CodeBlock => self.finish_code_block(),
            _ => {}
        }
    }

    fn finish_code_block(&mut self) {
        let Some((language, code)) = self.code_block.take() else {
            return;
        };
        if self
            .document
            .lines
            .last()
            .is_some_and(|line| line.width() > 0)
        {
            self.document.newline();
        }
        let code = code.strip_suffix('\n').unwrap_or(&code);
        self.document.append(
            CodeHighlighter::global()
                .code(&language, code)
                .gutter("│ ", muted()),
        );
        self.document.newline();
        self.document.newline();
    }

    fn with_style(&mut self, update: impl FnOnce(&mut TextStyle)) {
        self.styles.push(self.style);
        update(&mut self.style);
    }
}

#[cfg(test)]
mod tests {
    use crossterm::style::Color;

    use super::Markdown;

    #[test]
    fn renders_markdown_structure_and_inline_styles() {
        let document =
            Markdown::render("# Heading\n\n- **bold** and `code`\n\n```rust\nfn main() {}\n```");
        let text: String = document
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(!text.contains('#'));
        assert!(!text.contains("**"));
        assert!(
            document
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.style.bold)
        );
        assert!(
            document
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.text == "code" && span.style.foreground == Some(Color::Yellow)
                })
        );
        assert!(
            document
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.text.contains("fn")
                        && matches!(span.style.foreground, Some(Color::Rgb { .. }))
                })
        );
    }

    #[test]
    fn code_blocks_have_a_non_color_boundary_and_surrounding_space() {
        let document = Markdown::render("Before.\n\n```text\ncommand --flag\n```\n\nAfter.");
        let lines: Vec<String> = document
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.text.as_str()).collect())
            .collect();
        let code = lines
            .iter()
            .position(|line| line == "│ command --flag")
            .expect("framed code row");

        assert!(code > 0 && lines[code - 1].is_empty(), "{lines:#?}");
        assert!(
            lines.get(code + 1).is_some_and(String::is_empty),
            "{lines:#?}"
        );
    }
}
