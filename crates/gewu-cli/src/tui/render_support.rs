use crossterm::style::Color;
use gewu_review::{ChangePresentation, Illustration};

use super::style::{Document, TextStyle};

/// Marks every line that belongs to the reviewed source rather than to Gewu.
pub(super) const SOURCE_GUTTER: &str = "▏ ";

pub(super) fn document_title(title: &str) -> Document {
    let mut document = Document::new();
    document.push(title, title_style());
    document.newline();
    document.newline();
    document
}

pub(super) fn heading(document: &mut Document, heading: &str) {
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

pub(super) fn title_style() -> TextStyle {
    TextStyle {
        foreground: Some(Color::Cyan),
        bold: true,
        ..TextStyle::default()
    }
}

pub(super) fn muted() -> TextStyle {
    TextStyle {
        foreground: Some(Color::DarkGrey),
        ..TextStyle::default()
    }
}

pub(super) fn source_path_style() -> TextStyle {
    TextStyle {
        foreground: Some(Color::Grey),
        bold: true,
        ..TextStyle::default()
    }
}

/// Names the view a reviewer is looking at, so quoted material is never
/// mistaken for the review's own explanation.
pub(super) fn presentation_label(presentation: ChangePresentation) -> &'static str {
    match presentation {
        ChangePresentation::Diff => "diff",
        ChangePresentation::AddedCode => "added code",
        ChangePresentation::RenderedMarkdown => "document",
        ChangePresentation::SummaryOnly => "summary only",
    }
}

pub(super) fn bullets(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn illustration(illustration: &Illustration) -> String {
    match illustration {
        Illustration::BeforeAfter { before, after } => {
            format!("**Before:** {before}\n\n**After:** {after}")
        }
        Illustration::Flow { steps } => steps.join(" → "),
        Illustration::Example { input, output } => {
            format!("`{input}` → `{output}`")
        }
    }
}
