use crossterm::style::Color;
use gewu_review::{ChangePresentation, Illustration};

use super::style::{Document, TextStyle};

/// Marks every line that belongs to the reviewed source rather than to Gewu.
pub(crate) const SOURCE_GUTTER: &str = "▏ ";

/// The narrowest terminal that shows a context pane beside the code. Below it
/// the pane goes under the chunk: self-review's UI judge found a side pane at
/// 120 columns wrapping every comment of the code it explained.
pub(crate) const SIDE_PANE_MIN_WIDTH: usize = 140;

pub(crate) fn document_title(title: &str) -> Document {
    let mut document = Document::new();
    document.push(title, title_style());
    document.newline();
    document.newline();
    document
}

pub(crate) fn heading(document: &mut Document, heading: &str) {
    if document.lines.last().is_some_and(|line| line.width() > 0) {
        document.newline();
    }
    document.push(
        heading,
        TextStyle {
            bold: true,
            ..TextStyle::default()
        },
    );
    document.newline();
}

pub(crate) fn title_style() -> TextStyle {
    TextStyle {
        foreground: Some(Color::Cyan),
        bold: true,
        ..TextStyle::default()
    }
}

pub(crate) fn muted() -> TextStyle {
    TextStyle {
        foreground: Some(Color::DarkGrey),
        ..TextStyle::default()
    }
}

pub(crate) fn source_path_style() -> TextStyle {
    TextStyle {
        foreground: Some(Color::Grey),
        bold: true,
        ..TextStyle::default()
    }
}

/// Names the view a reviewer is looking at, so quoted material is never
/// mistaken for the review's own explanation.
pub(crate) fn presentation_label(presentation: ChangePresentation) -> &'static str {
    match presentation {
        ChangePresentation::Diff => "diff",
        ChangePresentation::AddedCode => "added code",
        ChangePresentation::RenderedMarkdown => "document",
        ChangePresentation::SummaryOnly => "summary only",
    }
}

pub(crate) fn bullets(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn illustration(illustration: &Illustration) -> String {
    match illustration {
        Illustration::BeforeAfter { before, after } => {
            format!("**Before:** {before}\n\n**After:** {after}")
        }
        // One step per row, so a map of several parts stays readable.
        Illustration::Flow { steps } => steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {step}", index + 1))
            .collect::<Vec<_>>()
            .join("\n"),
        Illustration::Example { input, output } => {
            format!("`{input}` → `{output}`")
        }
        Illustration::Diagram { text } => format!("```mermaid\n{}\n```", text.trim_end()),
    }
}
